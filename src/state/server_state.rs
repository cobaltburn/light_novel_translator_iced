use crate::{
    actions::trans_action::TransAction, controller::server::Server, state::translation_model::Page,
};
use iced::{Task, task::Handle};

#[derive(Default, Debug)]
pub struct ServerState {
    pub server: Server,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub api_key: String,
    pub handles: Vec<Handle>, // handles must be added with abort on drop
    pub settings: Settings,
}

impl ServerState {
    pub fn connected(&self) -> bool {
        self.server.connected()
    }

    pub fn collect_task(
        &mut self,
        current_page: &Page,
        model: &String,
        page: usize,
    ) -> Vec<Task<TransAction>> {
        current_page
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let server = self.server.clone();
                let settings = self.settings.clone();
                server.translate(model.clone(), section.clone(), page, part, settings)
            })
            .map(|task| add_handle(&mut self.handles, task))
            .collect()
    }

    pub fn translate_part(
        &mut self,
        section: String,
        model: String,
        page: usize,
        part: usize,
    ) -> Task<TransAction> {
        let server = self.server.clone();
        let settings = self.settings.clone();
        let task = server.translate(model, section, page, part, settings);
        let task = add_handle(&mut self.handles, task);

        task
    }
}

fn add_handle(handles: &mut Vec<Handle>, task: Task<TransAction>) -> Task<TransAction> {
    let (task, handle) = task.abortable();
    handles.push(handle.abort_on_drop());
    task
}

#[derive(Default, Debug, Clone)]
pub struct Settings {
    pub think: bool,
    pub pause: u64,
    pub tempature: f32,
}
