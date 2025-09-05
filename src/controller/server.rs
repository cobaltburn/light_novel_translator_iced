use crate::controller::error::Error;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};

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

    pub async fn translate(self, model: String) -> Result<ChatMessageResponseStream, Error> {
        let Server::Connected(ollama) = self else {
            return Err(Error::ServerError(String::from("server not connected")));
        };

        let stream: ChatMessageResponseStream =
            execute_translation(&ollama, model, Vec::new(), &None).await?;
        return Ok(stream);
    }
}

pub async fn execute_translation(
    ollama: &Ollama,
    model: String,
    section: Vec<String>,
    context: &Option<String>,
) -> Result<ChatMessageResponseStream, Error> {
    // let prompt = generate_priming_prompt(context);
    let prompt = String::from("Write me a short store no more then 100 words long");
    let mut messages = vec![ChatMessage::user(prompt)];
    let mut lines = section.into_iter().map(|c| ChatMessage::user(c)).collect();
    messages.append(&mut lines);

    let stream: ChatMessageResponseStream = ollama
        .send_chat_messages_stream(ChatMessageRequest::new(model.clone(), messages))
        .await?;

    Ok(stream)
}
