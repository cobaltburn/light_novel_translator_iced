use crate::{error::TaskResultExt, llm::StreamAction};
use iced::Task;
use rig::{
    conversation::Conversation,
    memory::{
        MemoryStore, apply_summary, load_async, save_async, summarized_prefix, summary_state,
    },
    message::{Message, UserContent},
};

pub const CONVERSATION_ID: &str = "translation";
const SUMMARY_MAX_BYTES: usize = 8 * 1024;
const TRUNCATED: &str = "[…truncated…]\n";

/// A conversation the agent loads before and saves after every run. Messages
/// older than the window are folded into a summary by [`compact_history`].
#[derive(Clone)]
pub struct SharedHistory {
    pub store: MemoryStore,
    pub window: usize,
}

/// `context_window` counts user/assistant pairs, so the window holds twice as many messages.
pub fn shared_history(context_window: usize) -> SharedHistory {
    SharedHistory {
        store: MemoryStore::default(),
        window: context_window * 2,
    }
}

pub fn seed_history<A: StreamAction>(history: SharedHistory, messages: Vec<Message>) -> Task<A> {
    update_history(history, |conversation, window| {
        conversation.extend(messages);
        compact(conversation, window);
    })
}

/// Folds messages that fell out of the window into the summary before the next run.
pub fn compact_history<A: StreamAction>(history: SharedHistory) -> Task<A> {
    update_history(history, compact)
}

fn update_history<A: StreamAction>(
    history: SharedHistory,
    update: impl FnOnce(&mut Conversation, usize) + Send + 'static,
) -> Task<A> {
    Task::future(async move {
        let mut conversation = load_async(&history.store, CONVERSATION_ID).await?;
        update(&mut conversation, history.window);
        save_async(&history.store, CONVERSATION_ID, &mut conversation).await
    })
    .ok_or_display(Into::into)
}

/// Replaces everything but the newest `window` messages with a plain-text summary
/// of them, carrying over the previous summary and capped at [`SUMMARY_MAX_BYTES`].
fn compact(conversation: &mut Conversation, window: usize) {
    let from = summarized_prefix(conversation);
    let messages = conversation.messages();
    let upto = messages.len().saturating_sub(window);
    if upto <= from {
        return;
    }

    let mut state = summary_state(conversation);
    let mut summary = state.summary;
    for line in messages[from..upto]
        .iter()
        .filter_map(|m| render_message(m))
    {
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str(&line);
    }
    state.summary = truncate_summary(summary);
    apply_summary(conversation, conversation.id(upto), &state);
}

/// Keeps the newest [`SUMMARY_MAX_BYTES`] of `summary`, marking the cut.
fn truncate_summary(summary: String) -> String {
    if summary.len() <= SUMMARY_MAX_BYTES {
        return summary;
    }
    let body = summary.strip_prefix(TRUNCATED).unwrap_or(&summary);
    let mut cut = body
        .len()
        .saturating_sub(SUMMARY_MAX_BYTES - TRUNCATED.len());
    while !body.is_char_boundary(cut) {
        cut += 1;
    }
    format!("{TRUNCATED}{}", &body[cut..])
}

fn render_message(message: &Message) -> Option<String> {
    let (role, text) = match message {
        Message::System { content } => ("system", content.clone()),
        Message::User { content } => {
            let parts: Vec<_> = content
                .iter()
                .filter_map(|part| match part {
                    UserContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect();
            ("user", parts.join(" "))
        }
        Message::Assistant(turn) => ("assistant", turn.text()),
    };
    (!text.is_empty()).then(|| format!("{role}: {text}"))
}

#[cfg(test)]
mod test {
    use super::{SUMMARY_MAX_BYTES, TRUNCATED, compact, truncate_summary};
    use rig::{conversation::Conversation, message::Message};

    fn pairs(range: std::ops::Range<usize>) -> Vec<Message> {
        range
            .flat_map(|i| {
                [
                    Message::user(format!("ja {i}")),
                    Message::assistant(format!("en {i}")),
                ]
            })
            .collect()
    }

    #[test]
    fn compact_keeps_window_and_summarizes_rest() {
        let mut conversation = Conversation::from_messages(pairs(0..3));
        compact(&mut conversation, 2);
        let messages = conversation.messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(*messages[1], Message::user("ja 2"));
        let Message::User { content } = &*messages[0] else {
            panic!("summary is a user message");
        };
        let summary = format!("{content:?}");
        assert!(summary.contains("user: ja 0") && summary.contains("assistant: en 1"));

        conversation.extend(pairs(3..4));
        compact(&mut conversation, 2);
        let messages = conversation.messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(*messages[1], Message::user("ja 3"));
        let summary = format!("{:?}", messages[0]);
        assert!(summary.contains("user: ja 0") && summary.contains("assistant: en 2"));
    }

    #[test]
    fn compact_within_window_is_noop() {
        let mut conversation = Conversation::from_messages(pairs(0..2));
        compact(&mut conversation, 4);
        assert_eq!(conversation.messages().len(), 4);
    }

    #[test]
    fn truncate_keeps_newest_bytes() {
        let summary = "あ".repeat(SUMMARY_MAX_BYTES);
        let truncated = truncate_summary(summary);
        assert!(truncated.starts_with(TRUNCATED));
        assert!(truncated.len() <= SUMMARY_MAX_BYTES);
        assert_eq!(truncate_summary(truncated.clone()), truncated);
    }
}
