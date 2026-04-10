use crate::{
    actions::{server_action::ServerAction, trans_action::TransAction},
    controller::client::{Client, SYSTEM_PROMPT},
    error::Result,
    model::translation::Page,
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use ollama_rs::generation::{chat::ChatMessage, parameters::ThinkType};
use std::{
    iter,
    sync::{Arc, Mutex},
};

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

    pub fn translate(
        &mut self,
        pages: &[Page],
        model: &String,
        page: usize,
    ) -> Result<Task<TransAction>> {
        match self.method {
            Method::History => self.translation_history(pages, model, page),
            _ => self.translation(pages, model, page),
        }
    }

    pub fn translation(
        &mut self,
        pages: &[Page],
        model: &String,
        page: usize,
    ) -> Result<Task<TransAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let client = self.client.clone();
                let settings = self.settings.clone();
                client.translate(model.clone(), section.clone(), page, part, settings)
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    pub fn translation_history(
        &mut self,
        pages: &[Page],
        model: &String,
        page: usize,
    ) -> Result<Task<TransAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let pages = pages.get(..pages.len()).unwrap_or_default();
        let history: Vec<_> = pages
            .into_iter()
            .map(|p| p.sections.iter().zip(p.text.iter()))
            .flatten()
            .filter(|e| !e.1.is_empty())
            .map(|(section, text)| {
                [
                    ChatMessage::user(text.clone()),
                    ChatMessage::assistant(section.clone()),
                ]
            })
            .collect();

        let len = history.len();
        let history = history
            .into_iter()
            .skip(len.saturating_sub(self.settings.context_window))
            .flatten();

        let history: Vec<_> = iter::once(ChatMessage::system(SYSTEM_PROMPT.to_owned()))
            .chain(history)
            .collect();

        let history = Arc::new(Mutex::new(history));

        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.clone().translate_history(
                    model.clone(),
                    section.clone(),
                    history.clone(),
                    page,
                    part,
                    self.settings.clone(),
                )
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    pub fn translate_part(
        &mut self,
        pages: &[Page],
        model: String,
        page: usize,
        part: usize,
    ) -> Result<Task<TransAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap().clone();

        let client = self.client.clone();
        let settings = self.settings.clone();

        let task = match self.method {
            Method::History => {
                let pages = pages.get(..pages.len()).unwrap_or_default();
                let section_history = current.sections.iter().zip(current.text.iter());

                let history: Vec<_> = pages
                    .into_iter()
                    .map(|p| p.sections.iter().zip(p.text.iter()))
                    .flatten()
                    .chain(section_history)
                    .filter(|e| !e.1.is_empty())
                    .map(|(section, text)| {
                        [
                            ChatMessage::user(text.clone()),
                            ChatMessage::assistant(section.clone()),
                        ]
                    })
                    .collect();

                let len = history.len();
                let history = history
                    .into_iter()
                    .skip(len.saturating_sub(self.settings.context_window))
                    .flatten();

                let history: Vec<_> = iter::once(ChatMessage::system(SYSTEM_PROMPT.to_owned()))
                    .chain(history)
                    .collect();

                let history = Arc::new(Mutex::new(history));
                client.translate_history(model, section, history, page, part, settings)?
            }
            _ => client.translate(model, section, page, part, settings)?,
        };

        let task = add_handle(&mut self.handles, task);

        Ok(task)
    }
}

pub fn add_handle(handles: &mut Vec<Handle>, task: Task<TransAction>) -> Task<TransAction> {
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
    pub think: Think,
    pub context_window: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            think: Default::default(),
            context_window: 10,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Think {
    #[default]
    High,
    Medium,
    Low,
    None,
}

impl Into<ThinkType> for Think {
    fn into(self) -> ThinkType {
        match self {
            Think::High => ThinkType::High,
            Think::Medium => ThinkType::Medium,
            Think::Low => ThinkType::Low,
            Think::None => ThinkType::False,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Method {
    #[default]
    Chain,
    Batch,
    History,
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
            Method::History | Method::Chain => {
                let mut chain = Task::none();
                for task in tasks {
                    chain = chain.chain(task);
                }
                chain
            }
        }
    }
}
