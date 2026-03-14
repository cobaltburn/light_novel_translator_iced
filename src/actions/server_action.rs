use crate::{
    controller::client::Client,
    message::display_error,
    model::server::{Method, Server, Think},
};
use iced::Task;
use ollama_rs::Ollama;

#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    SetMethod(Method),
    SetThink(Think),
    Connect,
    Abort,
}

impl Server {
    pub fn perform(&mut self, action: ServerAction) -> Task<ServerAction> {
        match action {
            ServerAction::SelectModel(model) => (self.current_model = Some(model)).into(),
            ServerAction::SetThink(think) => (self.settings.think = think).into(),
            ServerAction::SetMethod(method) => (self.method = method).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
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

    pub fn abort(&mut self) {
        self.handles.clear(); // handles must be added with abort on drop
    }
}
