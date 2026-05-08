use crate::{
    actions::{
        consensus_action::ConsensusAction, server_action::ServerAction, trans_action::TransAction,
    },
    controller::client::Client,
    error::{Error, Result},
    model::page::{Page, Section},
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use quick_xml::{Writer, events::BytesText};
use rig::message::Message;
use std::{
    collections::HashMap,
    ffi::OsStr,
    io::Cursor,
    iter,
    sync::{Arc, Mutex},
};

const BATCH_SIZE: usize = 6;
const DEFAULT_CONTEXT_WINDOW: usize = 5;

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
        model: &str,
        page: usize,
    ) -> Result<Task<TransAction>> {
        match self.method {
            Method::History => self.translation_history(pages, model, page),
            _ => self.translation(pages, model, page),
        }
    }

    fn translation(
        &mut self,
        pages: &[Page],
        model: &str,
        page: usize,
    ) -> Result<Task<TransAction>> {
        let current = pages.last().expect("dont pass an empty array");

        let handles = &mut self.handles;
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.translate(
                    section.japanese.clone(),
                    model,
                    self.settings.think,
                    page,
                    part,
                )
            })
            .map(|task| task.map(|task| bind(handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    fn translation_history(
        &mut self,
        pages: &[Page],
        model: &str,
        page: usize,
    ) -> Result<Task<TransAction>> {
        let history = build_history(pages, self.settings.context_window);
        let history = Arc::new(Mutex::new(history));

        let current = pages.last().expect("dont pass an empty array");
        let handles = &mut self.handles;
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.translate_history(
                    section.japanese.clone(),
                    model,
                    history.clone(),
                    self.settings.context_window,
                    self.settings.think,
                    page,
                    part,
                )
            })
            .map(|task| task.map(|task| bind(handles, task)))
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
        let current = pages.last().unwrap();
        let section = current.sections.get(part).unwrap().clone();

        let task = match self.method {
            Method::History => {
                let history = build_history(pages, self.settings.context_window);
                let history = Arc::new(Mutex::new(history));

                self.client.translate_history(
                    section.japanese.clone(),
                    &model,
                    history,
                    self.settings.context_window,
                    self.settings.think,
                    page,
                    part,
                )?
            }
            _ => self.client.translate(
                section.japanese.clone(),
                &model,
                self.settings.think,
                page,
                part,
            )?,
        };

        Ok(self.bind_handle(task))
    }

    pub fn consensus(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
    ) -> Result<Task<ConsensusAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = candidates
            .get(current.file_stem().unwrap_or_default())
            .ok_or(Error::GeneralError(String::from("missing candidate files")))?;

        let handles = &mut self.handles;
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
                let prompt = consensus_prompt(&section.japanese, &candidates)?;
                self.client
                    .consensus(prompt, model.to_string(), self.settings.think, page, part)
            })
            .map(|task| task.map(|task| bind(handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    pub fn consensus_part(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: String,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap();
        let page_candidates = candidates
            .get(&current.file_stem().unwrap_or_default())
            .ok_or(Error::GeneralError(String::from("missing candidate file")))?;
        let page_candidates: Vec<_> = page_candidates.iter().flat_map(|e| e.get(part)).collect();
        let prompt = consensus_prompt(&section.japanese, &page_candidates)?;

        let task =
            self.client
                .consensus(prompt, model.to_string(), self.settings.think, page, part)?;

        Ok(self.bind_handle(task))
    }
}

fn bind<T: 'static>(handles: &mut Vec<Handle>, task: Task<T>) -> Task<T> {
    let (task, handle) = task.abortable();
    handles.push(handle.abort_on_drop());
    task
}

fn build_history(pages: &[Page], context_window: usize) -> Vec<Message> {
    let (current, history_pages) = pages.split_last().expect("dont pass an empty array");

    let mut recent: Vec<&Section> = current
        .sections
        .iter()
        .rev()
        .chain(
            history_pages
                .iter()
                .rev()
                .flat_map(|p| p.sections.iter().rev()),
        )
        .filter(|s| !s.content.is_empty())
        .take(context_window)
        .collect();
    recent.reverse();

    recent
        .into_iter()
        .flat_map(Section::history_message)
        .collect()
}

pub fn consensus_prompt(section: &str, candidates: &[&String]) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer
        .create_element("current_task")
        .write_inner_content(|writer| {
            writer
                .create_element("source")
                .with_attribute(("lang", "ja"))
                .write_text_content(BytesText::new(section))?;

            writer
                .create_element("candidates")
                .write_inner_content(|writer| {
                    for (i, candidate) in candidates.iter().enumerate() {
                        let id = i.to_string();
                        writer
                            .create_element("candidate")
                            .with_attribute(("id", id.as_str()))
                            .write_text_content(BytesText::new(candidate))?;
                    }
                    Ok(())
                })?;

            Ok(())
        })?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

impl Server {
    pub fn model_pick_list(&self) -> Element<'_, ServerAction> {
        pick_list(
            self.models.as_slice(),
            self.current_model.as_ref(),
            ServerAction::SelectModel,
        )
        .width(250)
        .into()
    }

    pub fn bind_handle<T: 'static>(&mut self, task: Task<T>) -> Task<T> {
        bind(&mut self.handles, task)
    }

    pub fn copy(&self) -> Self {
        Self {
            client: self.client.clone(),
            models: self.models.clone(),
            current_model: self.current_model.clone(),
            settings: self.settings.clone(),
            method: self.method,
            handles: Vec::new(),
        }
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
            context_window: DEFAULT_CONTEXT_WINDOW,
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

impl std::fmt::Display for Think {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Think::High => f.write_str("high"),
            Think::Medium => f.write_str("medium"),
            Think::Low => f.write_str("low"),
            Think::None => f.write_str("false"),
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
    pub fn join_tasks<T: 'static>(self, tasks: Vec<Task<T>>) -> Task<T> {
        match self {
            Method::Batch => {
                let mut iter = tasks.into_iter();
                iter::from_fn(|| {
                    let chunk: Vec<_> = iter.by_ref().take(BATCH_SIZE).collect();
                    (!chunk.is_empty()).then_some(chunk)
                })
                .map(Task::batch)
                .fold(Task::none(), Task::chain)
            }
            Method::History | Method::Chain => tasks.into_iter().fold(Task::none(), Task::chain),
        }
    }
}
