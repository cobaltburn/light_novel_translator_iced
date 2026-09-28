use crate::{
    actions::{consensus, server, translation},
    controller::Client,
    error::{Error, Result},
    model::{Page, Section},
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use quick_xml::{Writer, events::BytesText};
use rig_core::message::Message;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::{
    collections::HashMap,
    ffi::OsStr,
    io::Cursor,
    iter,
    sync::{Arc, Mutex},
};

pub const TEMPERATURE: f64 = 0.8;
pub const TOP_P: f64 = 0.8;
pub const REPEAT_PENALTY: f64 = 1.05;
pub const BATCH_SIZE: usize = 6;
pub const DEFAULT_CONTEXT_WINDOW: usize = 3;

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
    ) -> Result<Task<translation::Action>> {
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
    ) -> Result<Task<translation::Action>> {
        let current = pages.last().expect("dont pass an empty array");

        let handles = &mut self.handles;
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client
                    .translate(&section.japanese, model, self.settings.clone(), page, part)
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
    ) -> Result<Task<translation::Action>> {
        let (current, pages) = pages.split_last().unwrap();
        let sections: Vec<_> = pages.iter().map(|p| p.sections.as_slice()).collect();
        let history = build_history(&sections, self.settings.context_window);
        let history = Arc::new(Mutex::new(history));

        let handles = &mut self.handles;
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.translate_history(
                    &section.japanese,
                    model,
                    history.clone(),
                    self.settings.clone(),
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
        model: &str,
        page: usize,
        part: usize,
    ) -> Result<Task<translation::Action>> {
        let (Page { sections, .. }, pages) = pages.split_last().unwrap();
        let (section, current_sections) = sections
            .get(..part + 1)
            .and_then(|p| p.split_last())
            .unwrap();
        let sections: Vec<_> = pages
            .iter()
            .map(|p| p.sections.as_slice())
            .chain(iter::once(current_sections))
            .collect();

        let task = match self.method {
            Method::History => {
                let history = build_history(&sections, self.settings.context_window);
                let history = Arc::new(Mutex::new(history));

                self.client.translate_history(
                    &section.japanese,
                    model,
                    history,
                    self.settings.clone(),
                    page,
                    part,
                )?
            }
            _ => self.client.translate(
                &section.japanese,
                model,
                self.settings.clone(),
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
    ) -> Result<Task<consensus::Action>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = candidates
            .get(current.file_stem().unwrap_or_default())
            .ok_or(Error::Error(String::from("missing candidate files")))?;

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
    ) -> Result<Task<consensus::Action>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap();
        let page_candidates = candidates
            .get(&current.file_stem().unwrap_or_default())
            .ok_or(Error::Error(String::from("missing candidate file")))?;
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

fn build_history(sections: &[&[Section]], context_window: usize) -> Vec<Message> {
    let mut recent: Vec<_> = sections
        .iter()
        .rev()
        .flat_map(|&p| p.iter().rev())
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
    pub fn model_pick_list(&self) -> Element<'_, server::Action> {
        pick_list(
            self.models.as_slice(),
            self.current_model.as_ref(),
            server::Action::SelectModel,
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
    pub temperature: f64,
    pub top_p: f64,
    pub repeat_penalty: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            think: Default::default(),
            context_window: DEFAULT_CONTEXT_WINDOW,
            temperature: TEMPERATURE,
            top_p: TOP_P,
            repeat_penalty: REPEAT_PENALTY,
        }
    }
}

impl Settings {
    pub fn agent_params(&self) -> Value {
        serde_json::json!({
            "top_p": &self.top_p,
            "repeat_penalty": &self.repeat_penalty,
            "think": &self.think,
        })
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

impl Serialize for Think {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Think::High => serializer.serialize_str("high"),
            Think::Medium => serializer.serialize_str("medium"),
            Think::Low => serializer.serialize_str("low"),
            Think::None => serializer.serialize_bool(false),
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
