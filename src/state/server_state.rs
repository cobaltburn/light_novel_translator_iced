use crate::{controller::server::Server, message::Message};
use iced::Task;
use ollama_rs::Ollama;

#[non_exhaustive]
#[derive(Default)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
}

impl ServerState {
    pub fn connect_server(&mut self, url: Option<String>) -> Task<Message> {
        let ollama = match url {
            None => Ollama::default(),
            Some(url) => match Ollama::try_new(url) {
                Ok(ollama) => ollama,
                Err(error) => {
                    log::error!("url parse error: {}", error);
                    return Task::none();
                }
            },
        };
        self.server = Server::Connected(ollama);

        Task::perform(self.server.clone().get_models(), |models| match models {
            Ok(models) => Message::SetModels(models),
            Err(error) => {
                log::error!("{}", error);
                Message::None
            }
        })
    }

    pub fn set_models(&mut self, models: Vec<String>) -> Task<Message> {
        self.current_model = models.first().cloned();
        self.models = models;
        Task::none()
    }

    pub fn set_current_model(&mut self, model: String) -> Task<Message> {
        self.current_model = Some(model);
        Task::none()
    }
}
