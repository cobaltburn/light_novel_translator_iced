use crate::{
    controller::Client,
    error::TaskResultExt,
    model::{Method, Server, Think},
};
use iced::Task;

#[derive(Debug, Clone)]
pub enum Action {
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
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::SelectModel(model) => self.set_model(model).into(),
            Action::SetThink(think) => self.set_think(think).into(),
            Action::SetMethod(method) => self.set_method(method).into(),
            Action::SetModels(models) => self.set_models(models).into(),
            Action::SetWindow(window) => self.set_window(window).into(),
            Action::Connect => self.connect(),
            Action::Abort => self.abort().into(),
            Action::SetTemp(temp) => self.set_temp(temp).into(),
            Action::SetTopP(top_p) => self.set_top_p(top_p).into(),
            Action::SetRepeatPenalty(penalty) => self.set_repeat_penalty(penalty).into(),
        }
    }

    pub fn connect(&mut self) -> Task<Action> {
        self.client = Client::ollama();
        let client = self.client.clone();
        Task::future(async move { client.get_models().await })
            .ok_or_display(|models| Task::done(Action::SetModels(models)))
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
