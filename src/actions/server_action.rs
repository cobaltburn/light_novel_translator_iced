use crate::{
    controller::client::Client,
    error::TaskResultExt,
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
    SetTemp(f64),
    SetTopP(f64),
    SetRepeatPenalty(f64),
    Connect,
    Abort,
}

impl Server {
    pub fn perform(&mut self, action: ServerAction) -> Task<ServerAction> {
        match action {
            ServerAction::SelectModel(model) => self.set_model(model).into(),
            ServerAction::SetThink(think) => self.set_think(think).into(),
            ServerAction::SetMethod(method) => self.set_method(method).into(),
            ServerAction::SetModels(models) => self.set_models(models).into(),
            ServerAction::SetWindow(window) => self.set_window(window).into(),
            ServerAction::Connect => self.connect(),
            ServerAction::Abort => self.abort().into(),
            ServerAction::SetTemp(temp) => self.set_temp(temp).into(),
            ServerAction::SetTopP(top_p) => self.set_top_p(top_p).into(),
            ServerAction::SetRepeatPenalty(penalty) => self.set_repeat_penalty(penalty).into(),
        }
    }

    pub fn connect(&mut self) -> Task<ServerAction> {
        self.client = Client::ollama();
        let client = self.client.clone();
        Task::future(async move { client.get_models().await })
            .ok_or_display(|models| Task::done(ServerAction::SetModels(models)))
    }

    fn set_model(&mut self, model: String) {
        self.current_model = Some(model)
    }

    fn set_method(&mut self, method: Method) {
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

    fn set_temp(&mut self, temp: f64) {
        self.settings.temperature = temp;
    }

    fn set_top_p(&mut self, top_p: f64) {
        self.settings.top_p = top_p;
    }

    fn set_repeat_penalty(&mut self, penalty: f64) {
        self.settings.repeat_penalty = penalty;
    }

    pub fn abort(&mut self) {
        self.handles.clear(); // handles must be added with abort on drop
    }
}
