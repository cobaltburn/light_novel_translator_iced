use crate::{
    actions::{
        consensus_action::ConsensusAction, server_action::ServerAction, trans_action::TransAction,
    },
    controller::client::{CONSENSUS_PROMPT, Client, TRANSLATION_PROMPT},
    error::Result,
    model::page::{Page, Section},
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use ollama_rs::generation::{
    chat::{ChatMessage, request::ChatMessageRequest},
    parameters::ThinkType,
};
use std::{
    borrow::Cow,
    collections::HashMap,
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

    fn translation(
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
                self.client.clone().translate(
                    ChatMessageRequest::new(
                        model.clone(),
                        vec![
                            ChatMessage::system(TRANSLATION_PROMPT.to_string()),
                            ChatMessage::user(section.japanese.clone()),
                        ],
                    )
                    .think(self.settings.think),
                    page,
                    part,
                )
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    fn translation_history(
        &mut self,
        pages: &[Page],
        model: &String,
        page: usize,
    ) -> Result<Task<TransAction>> {
        let (current, history_pages) = pages.split_last().expect("dont pass an empty array");

        let pairs: Vec<_> = history_pages
            .iter()
            .flat_map(|p| &p.sections)
            .filter(|s| !s.content.is_empty())
            .map(|Section { japanese, content }| {
                [
                    ChatMessage::user(japanese.clone()),
                    ChatMessage::assistant(content.clone()),
                ]
            })
            .collect();

        let skip = pairs.len().saturating_sub(self.settings.context_window);
        let history: Vec<_> = iter::once(ChatMessage::system(TRANSLATION_PROMPT.to_owned()))
            .chain(pairs.into_iter().skip(skip).flatten())
            .collect();

        let history = Arc::new(Mutex::new(history));

        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.clone().translate_history(
                    ChatMessageRequest::new(
                        model.clone(),
                        vec![ChatMessage::user(section.japanese.clone())],
                    )
                    .think(self.settings.think),
                    history.clone(),
                    page,
                    part,
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

        let messages = match self.method {
            Method::History => vec![ChatMessage::user(section.japanese.clone())],
            _ => vec![
                ChatMessage::system(TRANSLATION_PROMPT.to_string()),
                ChatMessage::user(section.japanese.clone()),
            ],
        };
        let request = ChatMessageRequest::new(model, messages).think(self.settings.think);

        let task = match self.method {
            Method::History => {
                let (_, history_pages) = pages.split_last().expect("dont pass an empty array");

                let pairs: Vec<_> = history_pages
                    .iter()
                    .flat_map(|p| &p.sections)
                    .chain(&current.sections)
                    .filter(|s| !s.content.is_empty())
                    .map(|Section { japanese, content }| {
                        [
                            ChatMessage::user(japanese.clone()),
                            ChatMessage::assistant(content.clone()),
                        ]
                    })
                    .collect();

                let skip = pairs.len().saturating_sub(self.settings.context_window);
                let history: Vec<_> =
                    iter::once(ChatMessage::system(TRANSLATION_PROMPT.to_owned()))
                        .chain(pairs.into_iter().skip(skip).flatten())
                        .collect();

                client.translate_history(request, Arc::new(Mutex::new(history)), page, part)?
            }
            _ => client.translate(request, page, part)?,
        };

        Ok(add_handle(&mut self.handles, task))
    }
}

impl Server {
    pub fn consensus(
        &mut self,
        pages: &[Page],
        candidates: HashMap<Cow<str>, Vec<&Vec<String>>>,
        model: &String,
        page: usize,
    ) -> Result<Task<ConsensusAction>> {
        match self.method {
            Method::History => self.consensus_history(pages, candidates, model, page),
            _ => self.consensus_single(pages, candidates, model, page),
        }
    }

    fn consensus_single(
        &mut self,
        pages: &[Page],
        candidates: HashMap<Cow<str>, Vec<&Vec<String>>>,
        model: &String,
        page: usize,
    ) -> Result<Task<ConsensusAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = candidates
            .get(&current.path.file_name().unwrap().to_string_lossy())
            .unwrap();
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
                let prompt = consensus_prompt(&section.japanese, &candidates);
                let request = ChatMessageRequest::new(
                    model.clone(),
                    vec![
                        ChatMessage::system(CONSENSUS_PROMPT.to_string()),
                        ChatMessage::user(prompt),
                    ],
                )
                .think(self.settings.think);
                self.client.clone().consensus(request, page, part)
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    fn consensus_history(
        &mut self,
        pages: &[Page],
        candidates: HashMap<Cow<str>, Vec<&Vec<String>>>,
        model: &String,
        page: usize,
    ) -> Result<Task<ConsensusAction>> {
        let (current, history_pages) = pages.split_last().expect("dont pass an empty array");
        let pairs: Vec<_> = history_pages
            .iter()
            .flat_map(|p| {
                let filename = p.path.file_name().unwrap().to_string_lossy();
                let page_candidates = candidates.get(&filename).unwrap();
                p.sections
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| !s.content.is_empty())
                    .map(move |(i, s)| {
                        let translations: Vec<_> =
                            page_candidates.iter().filter_map(|c| c.get(i)).collect();
                        let prompt = consensus_prompt(&s.japanese, &translations);
                        [
                            ChatMessage::user(prompt),
                            ChatMessage::assistant(s.content.clone()),
                        ]
                    })
            })
            .collect();

        let skip = pairs.len().saturating_sub(self.settings.context_window);
        let history: Vec<_> = iter::once(ChatMessage::system(CONSENSUS_PROMPT.to_owned()))
            .chain(pairs.into_iter().skip(skip).flatten())
            .collect();
        let history = Arc::new(Mutex::new(history));

        let candidates = candidates
            .get(&current.path.file_name().unwrap().to_string_lossy())
            .unwrap();

        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
                let prompt = consensus_prompt(&section.japanese, &candidates);
                self.client.clone().consensus_history(
                    ChatMessageRequest::new(model.clone(), vec![ChatMessage::user(prompt)])
                        .think(self.settings.think),
                    history.clone(),
                    page,
                    part,
                )
            })
            .map(|task| task.map(|task| add_handle(&mut self.handles, task)))
            .collect();

        Ok(self.method.join_tasks(tasks?))
    }

    pub fn consensus_part(
        &mut self,
        pages: &[Page],
        candidates: HashMap<Cow<str>, Vec<&Vec<String>>>,
        model: String,
        page: usize,
        part: usize,
    ) -> Result<Task<ConsensusAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let section = current.sections.get(part).unwrap().clone();
        let page_candidates = candidates
            .get(&current.path.file_name().unwrap().to_string_lossy())
            .unwrap();
        let page_candidates: Vec<_> = page_candidates.iter().flat_map(|e| e.get(part)).collect();
        let prompt = consensus_prompt(&section.japanese, &page_candidates);

        let messages = match self.method {
            Method::History => vec![ChatMessage::user(prompt)],
            _ => vec![
                ChatMessage::system(CONSENSUS_PROMPT.to_string()),
                ChatMessage::user(prompt),
            ],
        };
        let request = ChatMessageRequest::new(model, messages).think(self.settings.think);

        let task = match self.method {
            Method::History => {
                let (_, history_pages) = pages.split_last().expect("dont pass an empty array");

                let pairs: Vec<_> = history_pages
                    .iter()
                    .flat_map(|p| {
                        let filename = p.path.file_name().unwrap().to_string_lossy();
                        let page_candidates = candidates.get(&filename).unwrap();
                        p.sections
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| !s.content.is_empty())
                            .map(move |(i, s)| {
                                let translations: Vec<_> =
                                    page_candidates.iter().filter_map(|c| c.get(i)).collect();
                                let prompt = consensus_prompt(&s.japanese, &translations);
                                [
                                    ChatMessage::user(prompt),
                                    ChatMessage::assistant(s.content.clone()),
                                ]
                            })
                    })
                    .collect();

                let skip = pairs.len().saturating_sub(self.settings.context_window);
                let history: Vec<_> = iter::once(ChatMessage::system(CONSENSUS_PROMPT.to_owned()))
                    .chain(pairs.into_iter().skip(skip).flatten())
                    .collect();

                self.client.clone().consensus_history(
                    request,
                    Arc::new(Mutex::new(history)),
                    page,
                    part,
                )?
            }
            _ => self.client.clone().consensus(request, page, part)?,
        };

        Ok(add_handle(&mut self.handles, task))
    }
}

pub fn consensus_prompt(section: &str, candidates: &[&String]) -> String {
    let source = format!("<source lang=\"ja\">\n{}\n</source>", section);

    let candidates: String = candidates
        .into_iter()
        .enumerate()
        .map(|(i, e)| format!("<candidate id=\"{}\">\n{}\n</candidate>\n", i, e))
        .collect();

    format!("{}\n<candidates>\n{}</candidates>", source, candidates)
}

pub fn add_handle<T: 'static>(handles: &mut Vec<Handle>, task: Task<T>) -> Task<T> {
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
            context_window: 5,
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
    pub fn join_tasks<T: 'static>(&self, tasks: Vec<Task<T>>) -> Task<T> {
        const BATCH_SIZE: usize = 6;

        match self {
            Method::Batch => {
                let mut iter = tasks.into_iter();
                std::iter::from_fn(|| {
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
