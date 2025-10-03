use crate::controller::error::Error;
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, ChatMessageResponseStream, request::ChatMessageRequest},
};
use std::{
    ops::Not,
    sync::{Arc, Mutex},
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

    pub async fn translate(
        self,
        model: String,
        section: Vec<String>,
        history: Arc<Mutex<Vec<ChatMessage>>>,
    ) -> Result<ChatMessageResponseStream, Error> {
        let Server::Connected(ollama) = self else {
            return Err(Error::ServerError(String::from("server not connected")));
        };

        let messages = section.into_iter().map(|c| ChatMessage::user(c)).collect();
        let stream: ChatMessageResponseStream = ollama
            .send_chat_messages_with_history_stream(
                history,
                ChatMessageRequest::new(model.clone(), messages),
            )
            .await?;

        Ok(stream)
    }

    pub fn connected(&self) -> bool {
        matches!(self, Server::Disconnected).not()
    }
}
