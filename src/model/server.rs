use crate::{
    actions::{server_action::ServerAction, trans_action::TransAction},
    controller::client::Client,
    error::Result,
    model::translation::Page,
};
use iced::{Element, Task, task::Handle, widget::pick_list};

#[derive(Default, Debug)]
pub struct Server {
    pub client: Client,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub handles: Vec<Handle>, // handles must be added with abort on drop
    pub settings: Settings,
    pub method: Method,
}

impl Server {
    pub fn connected(&self) -> bool {
        self.client.connected()
    }

    pub fn translation_tasks(
        &mut self,
        current_page: &Page,
        model: &String,
        page: usize,
    ) -> Result<Vec<Task<TransAction>>> {
        current_page
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let server = self.client.clone();
                let settings = self.settings.clone();
                server.translate(model.clone(), section.clone(), page, part, settings)
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect()
    }

    pub fn translate_part(
        &mut self,
        section: String,
        model: String,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let server = self.client.clone();
        let settings = self.settings.clone();
        let task = server.translate(model, section, page, part, settings)?;
        let task = add_handle(&mut self.handles, task);

        Ok(task)
    }
}

fn add_handle(handles: &mut Vec<Handle>, task: Task<TransAction>) -> Task<TransAction> {
    let (task, handle) = task.abortable();
    handles.push(handle.abort_on_drop());
    task
}

impl Server {
    pub fn model_pick_list(&self) -> Element<'_, ServerAction> {
        pick_list(
            self.models.clone(),
            self.current_model.clone(),
            ServerAction::SelectModel,
        )
        .width(250)
        .into()
    }

    pub fn bind_handle<T>(&mut self, task: Task<T>) -> Task<T>
    where
        T: 'static,
    {
        let (task, handle) = task.abortable();
        self.handles.push(handle.abort_on_drop());
        task
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub think: bool,
    pub pause: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            think: true,
            pause: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Method {
    #[default]
    Chain,
    Batch,
}

impl Method {
    pub fn join_tasks(&self, tasks: Vec<Task<TransAction>>) -> Task<TransAction> {
        match self {
            Method::Batch => {
                let mut batch = Task::none();
                let mut tasks = tasks.into_iter();

                while let Some(task) = tasks.next() {
                    let mut chunk = vec![task];
                    for _ in 1..=5 {
                        if let Some(task) = tasks.next() {
                            chunk.push(task);
                        };
                    }
                    batch = batch.chain(Task::batch(chunk))
                }

                batch
            }
            Method::Chain => {
                let mut chain = Task::none();
                for task in tasks {
                    chain = chain.chain(task);
                }
                chain
            }
        }
    }
}
