use crate::error::{Error, Result};
use iced::Task;
use rig::{
    Agent, ErrorKind,
    agent::MultiTurnStreamItem,
    completion::FinishReason,
    message::Message,
    run::{CompletionCall, PromptError},
    streaming::{Item, StreamEvent},
};
use std::future;

pub trait StreamAction: 'static + Send + Clone {
    fn activity(page: usize, part: usize) -> Self;
    fn update(content: String, page: usize, part: usize) -> Self;
    fn fail(message: String) -> Self;
    fn clean(page: usize, part: usize) -> Self;
}

/// What a streaming reply produces. rig only errors on a truncated reply with
/// no text in it, so anything that needs the whole reply has to check.
pub enum Reply {
    /// A non-empty piece of the reply's text.
    Text(String),
    /// The reply hit a limit, so the text before this is not all of it.
    Truncated,
}

/// Marks the part active, runs `seed`, then streams the agent's reply.
pub fn stream_task<A: StreamAction>(
    agent: &Agent,
    seed: Task<A>,
    prompt: impl Into<Message>,
    page: usize,
    part: usize,
) -> Task<A> {
    Task::done(A::activity(page, part))
        .chain(seed)
        .chain(agent_stream(agent.clone(), prompt, page, part))
}

fn agent_stream<A: StreamAction>(
    agent: Agent,
    prompt: impl Into<Message>,
    page: usize,
    part: usize,
) -> Task<A> {
    reply_stream(agent, prompt)
        .then(move |reply| match reply {
            Ok(Reply::Text(text)) => Task::done(A::update(text, page, part)),
            Ok(Reply::Truncated) => Task::none(),
            Err(error) if is_response_report(&error) => error.display_error(),
            Err(error) => Task::done(A::clean(page, part)).chain(fail_run(error, A::fail)),
        })
        .chain(Task::done(A::clean(page, part)))
}

/// Prompts `agent` and yields its reply as it arrives.
pub fn reply_stream(agent: Agent, prompt: impl Into<Message>) -> Task<Result<Reply>> {
    let prompt = prompt.into();
    Task::future(async move { agent.prompt(prompt).stream() })
        .then(Task::stream)
        .map_err(Error::from)
        .and_then(|item| match item {
            MultiTurnStreamItem::StreamAssistantItem(Item::Event(StreamEvent::Text {
                text,
                ..
            })) if !text.is_empty() => Task::done(Ok(Reply::Text(text))),
            MultiTurnStreamItem::CompletionCall(call) if is_truncated(&call) => {
                Task::done(Ok(Reply::Truncated))
            }
            _ => Task::none(),
        })
}

fn is_truncated(call: &CompletionCall) -> bool {
    call.finish_reason
        .as_ref()
        .is_some_and(FinishReason::truncated_output)
}

/// Reports a fatal error, then waits to be aborted. Whatever is chained after
/// the failing stream must not run, and the handler of `failed` aborts the run
/// and shows the dialog, since a dialog bound to the run would die with it.
pub fn fail_run<T: Send + 'static>(error: Error, failed: impl FnOnce(String) -> T) -> Task<T> {
    log::error!("{error:#?}");
    Task::done(failed(error.to_string())).chain(Task::future(future::pending()))
}

fn is_response_report(error: &Error) -> bool {
    matches!(error, Error::PromptError(PromptError::Report(report)) if report.kind == ErrorKind::Response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::completion::Usage;
    use serde_json::Value;

    fn finished(reason: Option<FinishReason>) -> CompletionCall {
        CompletionCall::new(0, Usage::default(), Value::Null).with_finish_reason(reason)
    }

    #[test]
    fn a_reply_cut_off_by_the_context_limit_is_truncated() {
        assert!(is_truncated(&finished(Some(FinishReason::Length))));
    }

    #[test]
    fn a_reply_that_ended_or_gave_no_reason_is_not_truncated() {
        assert!(!is_truncated(&finished(Some(FinishReason::Stop))));
        assert!(!is_truncated(&finished(None)));
    }
}
