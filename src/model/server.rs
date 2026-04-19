use crate::{
    actions::{
        consensus_action::ConsensusAction, server_action::ServerAction, trans_action::TransAction,
    },
    controller::client::{CONSENSUS_PROMPT, Client, TRANSLATION_PROMPT},
    error::{Error, Result},
    model::page::{Page, Section},
};
use iced::{Element, Task, task::Handle, widget::pick_list};
use ollama_rs::generation::{
    chat::{ChatMessage, request::ChatMessageRequest},
    parameters::ThinkType,
};
use quick_xml::{Writer, events::BytesText};
use std::{
    collections::HashMap,
    ffi::OsStr,
    io::Cursor,
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
        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                self.client.clone().translate(
                    ChatMessageRequest::new(
                        model.to_string(),
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
        model: &str,
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
                        model.to_string(),
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
        candidates: HashMap<&OsStr, Vec<&[String]>>,
        model: &str,
        page: usize,
    ) -> Result<Task<ConsensusAction>> {
        let current = pages.last().expect("dont pass an empty array");
        let candidates = candidates
            .get(current.file_stem().unwrap())
            .ok_or(Error::GeneralError(String::from("missing candidate files")))?;

        let tasks: Result<Vec<_>> = current
            .sections
            .iter()
            .enumerate()
            .map(|(part, section)| {
                let candidates: Vec<_> = candidates.iter().flat_map(|e| e.get(part)).collect();
                let prompt = consensus_prompt(&section.japanese, &candidates)?;
                let request = ChatMessageRequest::new(
                    model.to_string(),
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
            .get(&current.file_stem().unwrap())
            .ok_or(Error::GeneralError(String::from("missing candidate file")))?;
        let page_candidates: Vec<_> = page_candidates.iter().flat_map(|e| e.get(part)).collect();
        let prompt = consensus_prompt(&section.japanese, &page_candidates)?;

        let task = self.client.clone().consensus(
            ChatMessageRequest::new(
                model,
                vec![
                    ChatMessage::system(CONSENSUS_PROMPT.to_string()),
                    ChatMessage::user(prompt),
                ],
            )
            .think(self.settings.think),
            page,
            part,
        )?;

        Ok(add_handle(&mut self.handles, task))
    }
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
