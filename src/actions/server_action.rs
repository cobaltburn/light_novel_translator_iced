use crate::{
    controller::client::Client,
    message::display_error,
    model::server::{Method, Server},
};
use iced::Task;
use ollama_rs::Ollama;

#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    Connect,
    SetMethod(Method),
    ThinkToggled(bool),
    SetPause(u64),
    Abort,
}

impl Server {
    pub fn perform(&mut self, action: ServerAction) -> Task<ServerAction> {
        match action {
            ServerAction::SelectModel(model) => self.set_current_model(model).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::ThinkToggled(toggled) => self.set_thinking(toggled).into(),
            ServerAction::SetPause(pause) => self.set_pause(pause).into(),
            ServerAction::SetMethod(method) => self.set_method(method).into(),
            ServerAction::Connect => self.connect(),
            ServerAction::Abort => self.abort().into(),
        }
    }

    pub fn connect(&mut self) -> Task<ServerAction> {
        self.client = Client::ollama(Ollama::default());
        Task::future(self.client.clone().get_models()).then(|models| match models {
            Ok(models) => Task::done(ServerAction::SetModels(models).into()),
            Err(error) => Task::future(display_error(error)).discard(),
        })
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.current_model = models.first().cloned();
        self.models = models;
    }

    pub fn set_method(&mut self, method: Method) {
        self.method = method;
    }

    pub fn set_current_model(&mut self, model: String) {
        self.current_model = Some(model);
    }

    pub fn abort(&mut self) {
        self.handles.clear(); // handles must be added with abort on drop
    }

    pub fn set_thinking(&mut self, toggled: bool) {
        self.settings.think = toggled
    }

    pub fn set_pause(&mut self, pause: u64) {
        self.settings.pause = pause;
    }
}
