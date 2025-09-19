use crate::controller::error::Error;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};
use std::ops::Not;

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub enum Server {
    #[default]
    Disconnected,
    Connected(Ollama),
}

impl Server {
    pub async fn get_models(self) -> Result<Vec<String>, Error> {
        let Server::Connected(server) = self else {
            return Err(Error::ServerError(String::from("server not connected")));
        };
        let models = server.list_local_models().await?;
        let models = models.into_iter().map(|model| model.name).collect();
        return Ok(models);
    }

    pub async fn translate(
        self,
        model: String,
        section: Vec<String>,
        context: Option<String>,
    ) -> Result<ChatMessageResponseStream, Error> {
        let Server::Connected(ollama) = self else {
            return Err(Error::ServerError(String::from("server not connected")));
        };

        let stream: ChatMessageResponseStream =
            execute_translation(&ollama, model, section, &context).await?;
        return Ok(stream);
    }

    pub fn connected(&self) -> bool {
        matches!(self, Server::Disconnected).not()
    }
}

pub async fn execute_translation(
    ollama: &Ollama,
    model: String,
    section: Vec<String>,
    context: &Option<String>,
) -> Result<ChatMessageResponseStream, Error> {
    let prompt = generate_priming_prompt(context);
    let mut messages = vec![ChatMessage::user(prompt)];
    let mut lines = section.into_iter().map(|c| ChatMessage::user(c)).collect();
    messages.append(&mut lines);

    let stream: ChatMessageResponseStream = ollama
        .send_chat_messages_stream(ChatMessageRequest::new(model.clone(), messages))
        .await?;

    Ok(stream)
}

pub fn generate_priming_prompt(context: &Option<String>) -> String {
    let mut prompt = String::from(
        r#"You are a highly skilled professional translator. You will be translating text from a Japanese book. You are a native speaker of English and Japanese. Translate the given text accurately, taking into account the context and specific instructions provided. If no additional instructions or context are provided, use your expertise to consider what the most appropriate context is and provide a natural translation that aligns with that context. When translating, strive to faithfully reflect the meaning and tone of the original text, pay attention to cultural nuances and differences in language usage, and ensure that the translation is grammatically correct and easy to read. For technical terms and proper nouns, either leave them in the original language or use appropriate translations as necessary. The translation should be complete. All the provide text should be translated. The translation should only contain the translated text. This should be a full translation of the given text. Do not summarize the text. Format the translated text into paragraphs with proper syntax. Match the paragraph structure of the original text. The text passed is in a markdown format the response should be in markdown. "#,
    );

    if let Some(context) = context {
        let context = format!(
            "Here is context about the story of the book, {}.\n",
            context
        );
        prompt.push_str(&context);
    }
    prompt.push_str("Take a deep breath, calm down, and start translating.");

    return prompt;
}
