use crate::{
    actions::{
        consensus_action::ConsensusAction, extraction_action::ExtractAction,
        server_action::ServerAction, trans_action::TransAction,
    },
    error::{Error, Result},
    message::display_error,
    model::server::{Settings, Think},
};
use iced::Task;
use ollama_rs::{
    Ollama,
    error::OllamaError,
    generation::{
        chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
        completion::{GenerationResponseStream, request::GenerationRequest},
        images::Image,
    },
};
use std::{
    ops::Not,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time;
use tokio_stream::StreamExt;

const MAX_WAIT: Duration = Duration::from_millis(250);
const CHUNK_SIZE: usize = 25;

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Client {
    #[default]
    Disconnected,
    Ollama(Ollama),
}

impl Client {
    pub fn ollama(client: Ollama) -> Client {
        Client::Ollama(client)
    }

    pub async fn get_models(self) -> Result<Vec<String>> {
        let model = match self {
            Client::Ollama(client) => {
                let models = client.list_local_models().await?;
                models.into_iter().map(|model| model.name).collect()
            }
            Client::Disconnected => {
                return Err(Error::ServerError("server not connected"));
            }
        };
        Ok(model)
    }

    pub fn translate(
        self,
        request: ChatMessageRequest,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        if let Client::Disconnected = self {
            return Err(Error::ServerError("server disconnected"));
        }

        Ok(Self::run_chat_task(
            self.stream(request),
            move |content| TransAction::UpdateContent {
                content,
                page,
                part,
            },
            TransAction::CancelTranslate,
            TransAction::CleanText { page, part },
        ))
    }

    pub fn translate_history(
        self,
        request: ChatMessageRequest,
        history: Arc<Mutex<Vec<ChatMessage>>>,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        if let Client::Disconnected = self {
            return Err(Error::ServerError("server disconnected"));
        }

        Ok(Self::run_chat_task(
            self.stream_history(request, history),
            move |content| TransAction::UpdateContent {
                content,
                page,
                part,
            },
            TransAction::CancelTranslate,
            TransAction::CleanText { page, part },
        ))
    }

    pub fn consensus(
        self,
        request: ChatMessageRequest,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        if let Client::Disconnected = self {
            return Err(Error::ServerError("server disconnected"));
        }

        Ok(Self::run_chat_task(
            self.stream(request),
            move |content| ConsensusAction::UpdateContent {
                content,
                page,
                part,
            },
            ConsensusAction::CancelConsensus,
            ConsensusAction::CleanText { page, part },
        ))
    }

    pub fn consensus_history(
        self,
        request: ChatMessageRequest,
        history: Arc<Mutex<Vec<ChatMessage>>>,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        if let Client::Disconnected = self {
            return Err(Error::ServerError("server disconnected"));
        }

        Ok(Self::run_chat_task(
            self.stream_history(request, history),
            move |content| ConsensusAction::UpdateContent {
                content,
                page,
                part,
            },
            ConsensusAction::CancelConsensus,
            ConsensusAction::CleanText { page, part },
        ))
    }

    fn run_chat_task<A>(
        stream_future: impl Future<Output = Result<ChatMessageResponseStream>> + Send + 'static,
        mut on_content: impl FnMut(String) -> A + Send + 'static,
        cancel: A,
        clean: A,
    ) -> Task<A>
    where
        A: Send + Clone + 'static,
    {
        Task::future(stream_future)
            .then(|stream| match stream {
                Ok(stream) => Task::run(stream.chunks_timeout(CHUNK_SIZE, MAX_WAIT), |res| {
                    res.into_iter().collect::<std::result::Result<Vec<_>, ()>>()
                })
                .map_err(|_| Error::ServerError("Failed to read stream")),
                Err(error) => Task::done(Err(error)),
            })
            .then(move |response| match response {
                Ok(msg) => {
                    let content: String = msg
                        .into_iter()
                        .map_while(|e| (!e.done).then_some(e.message.content))
                        .collect();
                    if content.is_empty() {
                        Task::none()
                    } else {
                        Task::done(on_content(content))
                    }
                }
                Err(error) => {
                    Task::done(cancel.clone()).chain(Task::future(display_error(error)).discard())
                }
            })
            .chain(Task::done(clean))
    }

    async fn stream(self, request: ChatMessageRequest) -> Result<ChatMessageResponseStream> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let stream = loop {
            match client.send_chat_messages_stream(request.clone()).await {
                Ok(stream) => break stream,
                Err(OllamaError::Other(error)) if error.contains("503") => {
                    time::sleep(Duration::from_secs(10)).await
                }
                Err(OllamaError::Other(error)) if error.contains("429") => {
                    time::sleep(Duration::from_secs(10)).await
                }
                Err(error) => return Err(Error::OllamaError(error)),
            }
        };

        Ok(stream)
    }

    async fn stream_history(
        self,
        request: ChatMessageRequest,
        history: Arc<Mutex<Vec<ChatMessage>>>,
    ) -> Result<ChatMessageResponseStream> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let stream = client
            .send_chat_messages_with_history_stream(history.clone(), request.clone())
            .await?;

        Ok(stream)
    }
}

impl Client {
    pub fn extract_text(
        self,
        model: String,
        image_base64: String,
        page: usize,
        Settings { think, .. }: Settings,
    ) -> Task<ExtractAction> {
        Task::future(self.extract_stream(model, image_base64, think))
            .then(move |stream| match stream {
                Ok(stream) => {
                    Task::stream(stream).map_err(|_| Error::ServerError("Failed to read stream"))
                }
                Err(error) => Task::done(Err(error)),
            })
            .then(move |response| match response {
                Ok(responses) => {
                    let content = responses.into_iter().map(|r| r.response).collect();
                    Task::done(ExtractAction::UpdateContent { content, page })
                }
                Err(error) => Task::done(ServerAction::Abort.into())
                    .chain(Task::future(display_error(error)).discard()),
            })
    }

    async fn extract_stream(
        self,
        model: String,
        image_base64: String,
        think: Think,
    ) -> Result<GenerationResponseStream> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let request = GenerationRequest::new(model, EXTRACT_PROMPT)
            .add_image(Image::from_base64(image_base64))
            .think(think);

        let stream = loop {
            match client.generate_stream(request.clone()).await {
                Ok(stream) => break stream,
                Err(OllamaError::Other(error)) if error.contains("503") => {
                    time::sleep(Duration::from_secs(10)).await
                }
                Err(error) => return Err(Error::OllamaError(error)),
            }
        };

        Ok(stream)
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

pub const EXTRACT_PROMPT: &str = r#"
You are extracting Japanese text from a light novel page.

OUTPUT RULES:
- Return ONLY raw Japanese text
- No explanations, translations, descriptions, or English text

IGNORE:
- Running header at the top of the page (book/chapter title that appears alongside page numbers)
- Page numbers
- Do NOT ignore vertical column text even if it extends near the top of the page

READING ORDER:
Columns are vertical. Read right-to-left across the page:
1. Start at the RIGHTMOST column
2. Read top to bottom within each column
3. Move LEFT to the next column
4. Repeat until the LEFTMOST column

EXTRACTION:
- Include ALL body text from every column, including edges
- Preserve all punctuation: 。、！？「」『』（）―…
- For furigana above kanji, format as: 漢字《かんじ》
- If a character is unclear, infer from context

Begin output with the first character of the rightmost body text column.
"#;
