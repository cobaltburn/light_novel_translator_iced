use crate::{
    controller::client::Client,
    model::server::{Method, Server, Think},
};
use iced::Task;

#[derive(Debug, Clone)]
pub enum ServerAction {
    SelectModel(String),
    SetModels(Vec<String>),
    SetMethod(Method),
    SetThink(Think),
    SetWindow(usize),
    Connect,
    Abort,
}

impl Server {
    pub fn perform(&mut self, action: ServerAction) -> Task<ServerAction> {
        match action {
            ServerAction::SelectModel(model) => self.select_model(model).into(),
            ServerAction::SetThink(think) => self.set_think(think).into(),
            ServerAction::SetMethod(method) => self.select_method(method).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::SetWindow(window) => self.set_window(window).into(),
            ServerAction::Connect => self.connect(),
            ServerAction::Abort => self.abort().into(),
        }
    }

    pub fn connect(&mut self) -> Task<ServerAction> {
        self.client = Client::ollama();
        Task::future(self.client.clone().get_models()).then(|models| match models {
            Ok(models) => Task::done(ServerAction::SetModels(models).into()),
            Err(error) => error.display_error(),
        })
    }

    fn select_model(&mut self, model: String) {
        self.current_model = Some(model)
    }
    fn select_method(&mut self, method: Method) {
        self.method = method;
    }

    fn set_think(&mut self, think: Think) {
        self.settings.think = think
    }

    fn set_models(&mut self, models: Vec<String>) {
        self.current_model = models.first().cloned();
        self.models = models;
    }

    fn set_window(&mut self, window: usize) {
        self.settings.context_window = window;
    }

    pub fn abort(&mut self) {
        self.handles.clear(); // handles must be added with abort on drop
    }
}
