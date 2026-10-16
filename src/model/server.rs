use crate::{
    actions::{consensus, server, translation},
    controller::{Client, SharedHistory, StreamAction, agent_stream, seed_history, shared_history},
    error::{Error, Result},
    model::{Page, Section},
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use quick_xml::{Writer, events::BytesText};
use rig::{Agent, message::Message};
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::{collections::HashMap, ffi::OsStr, io::Cursor, iter};

pub const TEMPERATURE: f64 = 0.8;
pub const TOP_P: f64 = 0.8;
pub const REPEAT_PENALTY: f64 = 1.05;
pub const BATCH_SIZE: usize = 6;
pub const DEFAULT_CONTEXT_WINDOW: usize = 10;

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
        let (current, pages) = pages.split_last().expect("dont pass an empty array");
        let (seed, history) = self.seeded_history(newest_first(pages)).unzip();
        let agent = self
            .client
            .translation_agent(model, &self.settings, history)?;

        let handles = &mut self.handles;
        let seed = seed.map_or_default(|seed| bind(handles, seed));
        let tasks: Vec<_> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| stream_task(&agent, Task::none(), &section.japanese, page, part))
            .map(|task| bind(handles, task))
            .collect();

        Ok(seed.chain(self.method.join_tasks(tasks)))
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
        let recent = current_sections.iter().rev().chain(newest_first(pages));
        let (seed, history) = self.seeded_history(recent).unzip();
        let agent = self
            .client
            .translation_agent(model, &self.settings, history)?;
        let seed = seed.unwrap_or_default();
        let task = stream_task(&agent, seed, &section.japanese, page, part);

        Ok(self.bind_handle(task))
    }

    /// In History mode, returns a task seeding a new shared history from `recent`
    /// (sections ordered newest first) along with that history.
    fn seeded_history<'a>(
        &self,
        recent: impl Iterator<Item = &'a Section>,
    ) -> Option<(Task<translation::Action>, SharedHistory)> {
        if self.method != Method::History {
            return None;
        }
        let history = shared_history(self.settings.context_window);
        let messages = build_history(recent, self.settings.context_window);
        Some((seed_history(history.clone(), messages), history))
    }

    pub fn consensus(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
    ) -> Result<Task<consensus::Action>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = page_candidates(current, &candidates)?;
        let agent = self.client.consensus_agent(model, self.settings.think)?;

        let handles = &mut self.handles;
        let tasks = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                consensus_task(&agent, section, candidates, page, part)
                    .map(|task| bind(handles, task))
            })
            .collect::<Result<_>>()?;

        Ok(self.method.join_tasks(tasks))
    }

    pub fn consensus_part(
        &mut self,
        pages: &[Page],
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
        part: usize,
    ) -> Result<Task<consensus::Action>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap();
        let candidates = page_candidates(current, &candidates)?;
        let agent = self.client.consensus_agent(model, self.settings.think)?;
        let task = consensus_task(&agent, section, candidates, page, part)?;

        Ok(self.bind_handle(task))
    }
}

fn page_candidates<'a, 'b>(
    page: &Page,
    candidates: &'a HashMap<&OsStr, Vec<&'b [String]>>,
) -> Result<&'a [&'b [String]]> {
    candidates
        .get(page.file_prefix().unwrap_or_default())
        .map(Vec::as_slice)
        .ok_or(Error::Error(String::from("missing candidate files")))
}

fn consensus_task(
    agent: &Agent,
    section: &Section,
    candidates: &[&[String]],
    page: usize,
    part: usize,
) -> Result<Task<consensus::Action>> {
    let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
    let prompt = consensus_prompt(&section.japanese, &candidates)?;

    Ok(stream_task(agent, Task::none(), prompt, page, part))
}

/// Marks the part active, runs `seed`, then streams the agent's reply.
fn stream_task<A: StreamAction>(
    agent: &Agent,
    seed: Task<A>,
    prompt: impl Into<Message>,
    page: usize,
    part: usize,
) -> Task<A> {
    Task::done(A::activity(page, part))
        .chain(seed)
        .chain(agent_stream(agent.clone(), prompt, page, part))
}

fn bind<T: 'static>(handles: &mut Vec<Handle>, task: Task<T>) -> Task<T> {
    let (task, handle) = task.abortable();
    handles.push(handle.abort_on_drop());
    task
}

/// Sections of `pages` ordered newest first.
fn newest_first(pages: &[Page]) -> impl Iterator<Item = &Section> {
    pages.iter().rev().flat_map(|p| p.sections.iter().rev())
}

/// Builds history messages from the latest `context_window` translated sections,
/// given `recent` ordered newest first. Stops walking once the window is full.
fn build_history<'a>(
    recent: impl Iterator<Item = &'a Section>,
    context_window: usize,
) -> Vec<Message> {
    let recent: Vec<_> = recent
        .filter(|s| !s.content.is_empty())
        .take(context_window)
        .collect();

    recent
        .into_iter()
        .rev()
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
    History,
    Chain,
    Batch,
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
