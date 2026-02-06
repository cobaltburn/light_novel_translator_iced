use crate::{
    actions::{
        extraction_action::ExtractAction, server_action::ServerAction, trans_action::TransAction,
    },
    error::{Error, Result},
    message::display_error,
    model::server::Settings,
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
use std::{ops::Not, time::Duration};
use tokio::time;

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
        model: String,
        section: String,
        page: usize,
        part: usize,
        settings: Settings,
    ) -> Result<Task<TransAction>> {
        match self {
            Client::Ollama { .. } => {
                Ok(self.ollama_translate(model, section, page, part, settings))
            }
            Client::Disconnected => Err(Error::ServerError("server disconnected")),
        }
    }

    fn ollama_translate(
        self,
        model: String,
        section: String,
        page: usize,
        part: usize,
        Settings { think, .. }: Settings,
    ) -> Task<TransAction> {
        Task::future(self.translation_stream(model, section, think))
            .then(move |stream| match stream {
                Ok(stream) => {
                    Task::stream(stream).map_err(|_| Error::ServerError("Failed to read stream"))
                }
                Err(error) => Task::done(Err(error)),
            })
            .then(move |response| match response {
                Ok(msg) => Task::done(TransAction::UpdateContent {
                    content: msg.message.content,
                    page,
                    part,
                }),
                Err(error) => Task::done(ServerAction::Abort.into())
                    .chain(Task::future(display_error(error)).discard()),
            })
    }

    async fn translation_stream(
        self,
        model: String,
        section: String,
        think: bool,
    ) -> Result<ChatMessageResponseStream> {
        let Client::Ollama(client) = self else {
            return Err(Error::ServerError("server not connected"));
        };

        let messages = vec![
            ChatMessage::system(SYSTEM_PROMPT.to_string()),
            ChatMessage::user(section),
        ];

        let request = ChatMessageRequest::new(model.clone(), messages).think(think);

        let stream = loop {
            match client.send_chat_messages_stream(request.clone()).await {
                Ok(stream) => break stream,
                Err(OllamaError::Other(error)) if error.contains("503") => {
                    time::sleep(Duration::from_secs(10)).await
                }
                Err(error) => return Err(Error::OllamaError(error)),
            }
        };

        Ok(stream)
    }

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
        think: bool,
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

pub const SYSTEM_PROMPT: &str = r#"
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
