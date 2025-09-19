use crate::{controller::server::Server, message::Message};
use iced::Task;
use ollama_rs::Ollama;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub url: String,
    pub invalid_url: bool,
}

impl ServerState {
    pub fn connected(&self) -> bool {
        self.server.connected()
    }
}

impl ServerState {
    pub fn connect_server(&mut self, url: Option<String>) -> Task<Message> {
        let ollama = match url {
            None => Ollama::default(),
            Some(url) => match Ollama::try_new(url) {
                Ok(ollama) => ollama,
                Err(error) => {
                    log::error!("url parse error: {}", error);
                    self.invalid_url = true;
                    return Task::none();
                }
            },
        };
        self.server = Server::Connected(ollama);

        Task::future(self.server.clone().get_models())
            .and_then(|models| Task::done(ServerAction::SetModels(models).into()))
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

    pub fn edit_url(&mut self, url: String) -> Task<Message> {
        self.url = url;
        self.invalid_url = false;
        Task::none()
    }

    pub fn perform(&mut self, action: ServerAction) -> Task<Message> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model),
            ServerAction::SetModels(models) => self.set_models(models),
            ServerAction::Connect(url) => self.connect_server(url),
            ServerAction::EditUrl(url) => self.edit_url(url),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    Connect(Option<String>),
    EditUrl(String),
}
