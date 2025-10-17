use crate::{
    controller::server::{Connection, MODELS, Server},
    message::Message,
};
use iced::{Task, task::Handle};
use ollama_rs::Ollama;
use rig::providers::anthropic;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub url: String,
    pub invalid_url: bool,
    pub api_key: String,
    pub handles: Vec<Handle>,
}

impl ServerState {
    pub fn connected(&self) -> bool {
        self.server.connected()
    }

    pub fn connect(&mut self, connection: Connection) -> Task<Message> {
        match connection {
            Connection::Ollama(url) => self.connect_ollama(url),
            Connection::Claude(api_key) => self.connect_claude(api_key),
        }
    }
    pub fn connect_claude(&mut self, api_key: String) -> Task<Message> {
        let client = anthropic::Client::new(&api_key);
        self.server = Server::claude(client);

        let models = MODELS.into_iter().map(|&model| model.to_string()).collect();
        Task::done(ServerAction::SetModels(models).into())
    }

    pub fn connect_ollama(&mut self, url: String) -> Task<Message> {
        let ollama = if url.trim().is_empty() {
            Ollama::default()
        } else {
            match Ollama::try_new(url) {
                Ok(ollama) => ollama,
                Err(error) => {
                    self.invalid_url = true;
                    return Task::done(Message::Error(format!("{:#?}", error)));
                }
            }
        };
        self.server = Server::ollama(ollama);

        Task::future(self.server.clone().get_models())
            .and_then(|models| Task::done(ServerAction::SetModels(models).into()))
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.current_model = models.first().cloned();
        self.models = models;
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = Some(model);
    }

    pub fn edit_url(&mut self, url: String) {
        self.url = url;
        self.invalid_url = false;
    }

    pub fn edit_api_key(&mut self, key: String) {
        self.api_key = key;
    }

    pub fn abort(&mut self) {
        self.handles.iter().for_each(|handle| handle.abort());
        self.handles.clear();
        self.server.clear_history();
    }

    pub fn perform(&mut self, action: ServerAction) -> Task<Message> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::Connect(Connection::Ollama(url)) => self.connect_ollama(url),
            ServerAction::Connect(Connection::Claude(key)) => self.connect_claude(key),
            ServerAction::EditUrl(url) => self.edit_url(url).into(),
            ServerAction::EditApiKey(key) => self.edit_api_key(key).into(),
            ServerAction::Abort => self.abort().into(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    Connect(Connection),
    EditUrl(String),
    EditApiKey(String),
    Abort,
}
