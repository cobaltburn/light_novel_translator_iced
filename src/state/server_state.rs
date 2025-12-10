use crate::{
    controller::server::{Connection, Server},
    message::Message,
};
use genai::{Client, resolver::AuthData};
use iced::{Task, task::Handle};
use ollama_rs::Ollama;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub api_key: String,
    pub handles: Vec<Handle>,
    pub think: bool,
}

impl ServerState {
    pub fn connected(&self) -> bool {
        self.server.connected()
    }

    pub fn connect(&mut self, connection: Connection) -> Task<Message> {
        match connection {
            Connection::Ollama => self.connect_ollama(),
            Connection::Claude(api_key) => self.connect_claude(api_key),
        }
    }
    pub fn connect_claude(&mut self, api_key: String) -> Task<Message> {
        let client = Client::builder()
            .with_auth_resolver_fn(|_| Ok(Some(AuthData::from_single(api_key))))
            .build();
        self.server = Server::claude(client);
        Task::perform(self.server.clone().get_models(), |models| match models {
            Ok(models) => ServerAction::SetModels(models).into(),
            Err(error) => Message::Error(error.to_string()),
        })
    }

    pub fn connect_ollama(&mut self) -> Task<Message> {
        self.server = Server::ollama(Ollama::default());

        Task::perform(self.server.clone().get_models(), |models| match models {
            Ok(models) => ServerAction::SetModels(models).into(),
            Err(error) => Message::Error(error.to_string()),
        })
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.current_model = models.first().cloned();
        self.models = models;
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = Some(model);
    }

    pub fn edit_api_key(&mut self, key: String) {
        self.api_key = key;
    }

    pub fn abort(&mut self) {
        self.handles.iter().for_each(|handle| handle.abort());
        self.handles.clear();
        self.server.clear_history();
    }

    pub fn set_thinking(&mut self, toggled: bool) {
        self.think = toggled
    }

    pub fn perform(&mut self, action: ServerAction) -> Task<Message> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::Connect(Connection::Ollama) => self.connect_ollama(),
            ServerAction::Connect(Connection::Claude(key)) => self.connect_claude(key),
            ServerAction::EditApiKey(key) => self.edit_api_key(key).into(),
            ServerAction::ThinkingToggled(toggled) => self.set_thinking(toggled).into(),
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
    EditApiKey(String),
    ThinkingToggled(bool),
    Abort,
}
