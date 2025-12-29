use crate::actions::trans_action::TransAction;
use crate::state::server_state::ServerState;
use crate::{
    controller::server::{Connection, Server},
    message::display_error,
};
use genai::{Client, resolver::AuthData};
use iced::Task;
use ollama_rs::Ollama;

#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    Connect(Connection),
    EditApiKey(String),
    ThinkToggled(bool),
    SetPause(u64),
    Abort,
}

impl ServerState {
    pub fn perform(&mut self, action: ServerAction) -> Task<TransAction> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::Connect(Connection::Ollama) => self.connect_ollama(),
            ServerAction::Connect(Connection::Claude(key)) => self.connect_claude(key),
            ServerAction::EditApiKey(key) => self.edit_api_key(key).into(),
            ServerAction::ThinkToggled(toggled) => self.set_thinking(toggled).into(),
            ServerAction::SetPause(pause) => self.set_pause(pause).into(),
            ServerAction::Abort => self.abort().into(),
        }
    }

    pub fn connect(&mut self, connection: Connection) -> Task<TransAction> {
        match connection {
            Connection::Ollama => self.connect_ollama(),
            Connection::Claude(api_key) => self.connect_claude(api_key),
        }
    }
    pub fn connect_claude(&mut self, api_key: String) -> Task<TransAction> {
        let client = Client::builder()
            .with_auth_resolver_fn(|_| Ok(Some(AuthData::from_single(api_key))))
            .build();
        self.server = Server::claude(client);
        Task::future(self.server.clone().get_models()).then(|models| match models {
            Ok(models) => Task::done(ServerAction::SetModels(models).into()),
            Err(error) => Task::future(display_error(error)).discard(),
        })
    }

    pub fn connect_ollama(&mut self) -> Task<TransAction> {
        self.server = Server::ollama(Ollama::default());
        Task::future(self.server.clone().get_models()).then(|models| match models {
            Ok(models) => Task::done(ServerAction::SetModels(models).into()),
            Err(error) => Task::future(display_error(error)).discard(),
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
        // handles must be added with abort on drop
        self.handles.clear();
        self.server.clear_history();
    }

    pub fn set_thinking(&mut self, toggled: bool) {
        self.settings.think = toggled
    }

    pub fn set_pause(&mut self, pause: u64) {
        self.settings.pause = pause;
    }
}
