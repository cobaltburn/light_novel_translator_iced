use crate::{
    actions::{consensus_action::ConsensusAction, trans_action::TransAction},
    error::{Error, Result},
    model::server::Think,
};
use iced::Task;
use reqwest_middleware::{ClientBuilder as MiddlewareBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use rig::{
    agent::{MultiTurnStreamItem, StreamingError},
    client::{CompletionClient, ModelListingClient, Nothing},
    completion::CompletionError,
    message::Message,
    providers::ollama::{self, OllamaApiKey, OllamaExt},
    streaming::{StreamedAssistantContent, StreamingChat, StreamingPrompt},
};
use std::{
    ops::Not,
    result,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_stream::StreamExt;

const MAX_WAIT: Duration = Duration::from_millis(250);
const CHUNK_SIZE: usize = 50;

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Client {
    #[default]
    Disconnected,
    Ollama(rig::client::Client<OllamaExt, ClientWithMiddleware>),
}

impl Client {
    pub fn ollama() -> Client {
        let policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(5), Duration::from_secs(30 * 60))
            .build_with_total_retry_duration(Duration::from_secs(30));
        let http = MiddlewareBuilder::new(Default::default())
            .with(RetryTransientMiddleware::new_with_policy(policy))
            .build();
        let client = ollama::Client::builder()
            .api_key(OllamaApiKey::from(Nothing))
            .http_client(http)
            .build()
            .unwrap();
        Client::Ollama(client)
    }

    pub async fn get_models(self) -> Result<Vec<String>> {
        match self {
            Client::Ollama(client) => {
                let models = client.list_models().await?;
                let mut models: Vec<_> = models.into_iter().map(|model| model.id).collect();
                models.sort();
                Ok(models)
            }
            Client::Disconnected => Err(Error::ServerError("server not connected")),
        }
    }

    pub fn translate(
        self,
        prompt: String,
        model: String,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let agent = client
            .agent(model)
            .preamble(TRANSLATION_PROMPT)
            .temperature(0.3)
            .additional_params(serde_json::json!({
                "num_ctx": 8192,
                "top_p": 0.8,
                "repeat_penalty": 1.05,
                "think": !matches!(think, Think::None),
            }))
            .build();

        let task = Task::future(async move { agent.stream_prompt(prompt).await })
            .then(|stream| {
                Task::run(stream.chunks_timeout(CHUNK_SIZE, MAX_WAIT), |r| {
                    r.into_iter()
                        .filter_map(|e| match e {
                            Err(StreamingError::Completion(CompletionError::JsonError(_))) => None,
                            e => Some(e),
                        })
                        .collect::<result::Result<Vec<_>, _>>()
                })
            })
            .and_then(move |items| {
                let content = items
                    .into_iter()
                    .flat_map(|r| match r {
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) if t.text.is_empty() => None,
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) => Some(t.text),
                        _ => None,
                    })
                    .collect::<String>();
                Task::done(Ok(content))
            })
            .then(move |content| match content {
                Ok(content) if content.is_empty() => Task::none(),
                Ok(content) => Task::done(TransAction::UpdateContent {
                    content,
                    page,
                    part,
                }),
                Err(error) => {
                    log::error!("{:#?}", error);
                    Task::done(TransAction::CancelTranslate)
                        .chain(Error::from(error).display_error())
                }
            })
            .chain(Task::done(TransAction::CleanText { page, part }));

        Ok(task)
    }

    pub fn translate_history(
        self,
        prompt: String,
        model: String,
        history: Arc<Mutex<Vec<Message>>>,
        context_window: usize,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let agent = client
            .agent(model)
            .preamble(TRANSLATION_PROMPT)
            .temperature(0.3)
            .additional_params(serde_json::json!({
                "num_ctx": 8192,
                "top_p": 0.8,
                "repeat_penalty": 1.05,
                "think": !matches!(think, Think::None),
            }))
            .build();

        let chat_history = history.clone();
        let task =
            Task::future(async move {
                let chat_history = chat_history.lock().unwrap().to_vec();
                agent.stream_chat(prompt, chat_history).await
            })
            .then(|stream| {
                Task::run(stream.chunks_timeout(CHUNK_SIZE, MAX_WAIT), |r| {
                    r.into_iter()
                        .filter_map(|e| match e {
                            Err(StreamingError::Completion(CompletionError::JsonError(_))) => None,
                            e => Some(e),
                        })
                        .collect::<result::Result<Vec<_>, _>>()
                })
            })
            .and_then(move |items| {
                let content = items
                    .into_iter()
                    .flat_map(|r| match r {
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) if t.text.is_empty() => None,
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) => Some(t.text),
                        MultiTurnStreamItem::FinalResponse(response) => {
                            if let Some(hist) = response.history() {
                                let mut shared = history.lock().unwrap();
                                shared.extend_from_slice(hist);
                                Self::shift_history(&mut shared, context_window);
                            }
                            None
                        }
                        _ => None,
                    })
                    .collect::<String>();
                Task::done(Ok(content))
            })
            .then(move |content| match content {
                Ok(content) if content.is_empty() => Task::none(),
                Ok(content) => Task::done(TransAction::UpdateContent {
                    content,
                    page,
                    part,
                }),
                Err(error) => Task::done(TransAction::CancelTranslate)
                    .chain(Error::from(error).display_error()),
            })
            .chain(Task::done(TransAction::CleanText { page, part }));

        Ok(task)
    }

    pub fn consensus(
        self,
        prompt: String,
        model: String,
        think: Think,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let agent = client
            .agent(model)
            .preamble(CONSENSUS_PROMPT)
            .temperature(0.3)
            .additional_params(serde_json::json!({
                "num_ctx": 8192,
                "top_p": 0.8,
                "repeat_penalty": 1.05,
                "think": !matches!(think, Think::None),
            }))
            .build();

        let task = Task::future(async move { agent.stream_prompt(prompt).await })
            .then(|stream| {
                Task::run(stream.chunks_timeout(CHUNK_SIZE, MAX_WAIT), |r| {
                    r.into_iter()
                        .filter_map(|e| match e {
                            Err(StreamingError::Completion(CompletionError::JsonError(_))) => None,
                            e => Some(e),
                        })
                        .collect::<result::Result<Vec<_>, _>>()
                })
            })
            .and_then(move |items| {
                let content = items
                    .into_iter()
                    .flat_map(|r| match r {
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) if t.text.is_empty() => None,
                        MultiTurnStreamItem::StreamAssistantItem(
                            StreamedAssistantContent::Text(t),
                        ) => Some(t.text),
                        _ => None,
                    })
                    .collect::<String>();
                Task::done(Ok(content))
            })
            .then(move |content| match content {
                Ok(content) if content.is_empty() => Task::none(),
                Ok(content) => Task::done(ConsensusAction::UpdateContent {
                    content,
                    page,
                    part,
                }),
                Err(error) => {
                    log::error!("{:#?}", error);
                    Task::done(ConsensusAction::CancelConsensus)
                        .chain(Error::from(error).display_error())
                }
            })
            .chain(Task::done(ConsensusAction::CleanText { page, part }));

        Ok(task)
    }

    fn shift_history(history: &mut Vec<Message>, context_window: usize) {
        let pair_count = history.len() / 2;
        if pair_count <= context_window {
            return;
        }
        let skip = (pair_count - context_window) * 2;
        history.drain(..skip);
    }

    pub fn connected(&self) -> bool {
        matches!(self, Client::Disconnected).not()
    }
}

pub const TRANSLATION_PROMPT: &str = r#"
You are an expert Japanese-to-English light novel translator. Translate the provided text completely and naturally.

## Core Requirements

- Translate ALL text - every sentence, every line of dialogue, every description
- Output ONLY the English translation - no commentary, notes, or explanations
- Match the paragraph structure of the source

## Output Language

All output must be in English. Never include Japanese characters in your response. If you encounter text you're uncertain how to translate, make your best interpretation - do not leave it untranslated.

## Translation Approach

- Preserve the author's voice, tone, and stylistic choices
- Render dialogue naturally while maintaining character voice
- Adapt idioms and cultural references for English readers when the literal meaning would be confusing
- Translate sound effects descriptively when onomatopoeia doesn't work in English

## Light Novel Conventions

- Maintain the light, readable prose style characteristic of the genre
- Preserve ellipses (…) for trailing thoughts and dramatic pauses
- Use em-dashes (—) for interrupted speech
- Keep the narrative energy and pacing of the original

## Internal Monologue

- Render character thoughts in italics when they appear as direct internal speech
- Maintain the distinction between narration and internal monologue present in the source

## Formatting Preservation

- Maintain line breaks where they appear in dialogue or for dramatic effect
- Preserve paragraph breaks exactly as they appear in the source
- Keep emphasis markers (if the source uses special formatting for emphasis, reflect it)

## Difficult Content Handling

- Wordplay/puns: Translate for equivalent effect in English, or translate the surface meaning if no equivalent exists
- Song lyrics or poetry: Maintain verse structure, prioritize meaning over rhyme
- Made-up terms/magic systems: Translate component kanji meanings into natural English equivalents
- Character name meanings: Keep the Japanese name, do not translate unless it's clearly a title or descriptor

## When Uncertain

If any passage is ambiguous, translate it based on context and light novel genre conventions. Never skip content, never leave Japanese text untranslated, never insert translator notes. Your output should read as if it were originally written in English.

Do not summarize. Do not describe what happens. Translate the actual words on the page.
"#;

pub const CONSENSUS_PROMPT: &str = r#"
You are an expert literary translator specializing in Japanese light novels. Your task is NOT to translate from scratch—you will receive a Japanese source passage and multiple candidate English translations from different models. Your job is to synthesize a single final translation that represents the best possible rendering of the source, drawing selectively from the candidates and correcting them where needed.

# Inputs

You will receive:
1. JAPANESE SOURCE: The original passage.
2. CANDIDATES: Numbered English translations (CANDIDATE 1, CANDIDATE 2, etc.) from different translation models.

# Your Process

Work through these steps internally before producing output:

1. **Read the Japanese source carefully.** Identify sentence boundaries, speakers, tense, register (formal/casual/archaic), and any culturally specific elements (honorifics, sound effects, wordplay, names).

2. **Compare candidates sentence by sentence.** For each sentence in the source, identify what each candidate did and where they agree or disagree.

3. **Resolve disagreements using this priority order:**
   a. **Fidelity to the source** — which candidate most accurately conveys the literal meaning, including subtle nuances of the Japanese?
   b. **Completeness** — which candidate preserves all information without summarizing, condensing, or omitting? Reject any candidate that has clearly shortened the source.
   c. **Natural English prose** — among accurate candidates, which reads most naturally as English literary fiction?
   d. **Voice and register consistency** — does the choice fit the speaker's established voice and the scene's tone?

4. **Synthesize, don't just pick.** You may take sentence A from CANDIDATE 1, sentence B from CANDIDATE 3, and rewrite sentence C entirely if all candidates failed it. The final output should be coherent and consistent in voice across the synthesis points.

5. **Correct shared errors.** If all candidates make the same mistake (mistranslation, wrong subject, dropped nuance), fix it based on the source. Consensus among candidates is a signal, not a mandate.

# Hard Rules

- **Name order:** Keep Japanese name order (family name first) unless the STYLE GUIDE says otherwise.
- **No summarization or condensation.** The output must reflect the full content and length of the source. If candidates have shortened things, restore the missing material from the source. Light novel prose is often deliberately verbose, repetitive, or meandering—preserve that.
- **No additions.** Do not insert explanatory phrases, cultural notes, or content not present in the source.
- **Sound effects and onomatopoeia:** Render naturally in English where possible; otherwise transliterate. Be consistent with whatever convention the candidates establish if it's reasonable.
- **Dialogue formatting:** Match the source's quotation/bracket style as rendered in the candidates (typically 「」 → "" for English).
- **Internal monologue, italics, emphasis:** Preserve formatting cues from the source.

# Pronoun and Subject Handling

You must NOT introduce pronouns or subjects that do not appear in any of the candidate translations. If all candidates use "she," you use "she." If candidates disagree on a pronoun (e.g., "he" vs "she" vs "they"), resolve it by checking the Japanese source. If the source is ambiguous (dropped subject), prefer whichever pronoun the majority of candidates used. Never substitute a pronoun based on your own interpretation of the source if the candidates already agree.

# Scope

You are translating ONE passage at a time. Each request contains a single source passage and its candidate translations. Your output must contain ONLY the translation of the current passage—never include or repeat translations from prior passages. That context exists solely to help you maintain consistency in voice, terminology, and pronouns. Do not reproduce it.

# When Candidates Conflict

- If candidates disagree on **who is speaking or acting**, return to the Japanese source and determine the correct subject. Japanese frequently drops subjects—use context.
- If candidates disagree on **tense**, default to what the Japanese grammar indicates, not what sounds smoother in English.
- If candidates disagree on a **specific word or term**, prefer the more precise or evocative choice that fits the register. Avoid generic substitutions.
- If one candidate is clearly an outlier (much shorter, missing sentences, hallucinated content), discount it heavily but still check if it caught something the others missed.
- If all candidates are poor for a given sentence, translate it yourself directly from the Japanese.

# Output Format

Output ONLY the final synthesized English translation. Do not include:
- Translations of prior passages
- Commentary on your choices
- Notes about which candidate you drew from
- Confidence scores
- The Japanese source
- Any preamble or explanation

The output should be ready to drop directly into the final manuscript.
"#;
