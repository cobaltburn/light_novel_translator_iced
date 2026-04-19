use crate::{
    actions::{
        clean_invisible_chars, complete_dialog, get_pages, pick_save_folder, save_file,
        select_format_folder, server_action::ServerAction,
    },
    controller::{parse::remove_think_tags, part_tag},
    error::{Error, Result},
    message::{display_error, select_epub},
    model::{
        Activity,
        consensus::{Candidate, Consensus},
        page::Page,
    },
};
use iced::Task;
use regex::Regex;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};
use tokio::fs;

#[derive(Debug, Clone)]
pub enum ConsensusAction {
    ServerAction(ServerAction),
    UpdateContent {
        content: String,
        page: usize,
        part: usize,
    },
    PageComplete(usize),
    Consensus(usize),
    ConsensusPage(usize),
    ConsensusPart {
        page: usize,
        part: usize,
    },
    CancelConsensus,
    SaveTranslation(String),
    SavePage {
        name: String,
        page: usize,
    },
    SetPage(usize),
    SavePages(PathBuf),
    SetEpub {
        path: PathBuf,
        pages: Vec<Page>,
    },
    OpenEpub,
    CleanText {
        page: usize,
        part: usize,
    },
    SelectCandidate(Option<usize>),
    SetCandidate {
        i: Option<usize>,
        name: String,
        pages: Vec<(PathBuf, String)>,
    },
    DropCandidate(usize),
}

impl Consensus {
    pub fn perform(&mut self, action: ConsensusAction) -> Task<ConsensusAction> {
        match action {
            ConsensusAction::ServerAction(action) => self.server.perform(action).map(Into::into),
            ConsensusAction::Consensus(page) => self
                .consensus(page)
                .unwrap_or_else(|error| Task::future(display_error(error)).discard()),
            ConsensusAction::ConsensusPage(page) => self
                .consensus_page(page)
                .unwrap_or_else(|error| Task::future(display_error(error)).discard()),
            ConsensusAction::ConsensusPart { page, part } => self
                .consensus_part(page, part)
                .unwrap_or_else(|error| Task::future(display_error(error)).discard()),
            ConsensusAction::SaveTranslation(file_name) => {
                Task::future(pick_save_folder(file_name))
                    .and_then(|path| {
                        Task::future(async { fs::create_dir(&path).await.map(|_| path) })
                    })
                    .then(|path| match path {
                        Ok(path) => Task::done(ConsensusAction::SavePages(path).into()),
                        Err(err) => Task::future(display_error(err)).discard(),
                    })
            }
            ConsensusAction::SavePages(path) => self.save_pages(path),
            ConsensusAction::SavePage { name, page } => self.save_page(name, page),
            ConsensusAction::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            ConsensusAction::PageComplete(page) => self.check_complete(page).into(),
            ConsensusAction::SetEpub { path: name, pages } => self.set_epub(name, pages).into(),
            ConsensusAction::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .then(|doc| match doc {
                    Ok((name, pages)) => Task::done(ConsensusAction::SetEpub { path: name, pages }),
                    Err(error) => Task::future(display_error(error)).discard(),
                }),
            ConsensusAction::CancelConsensus => self.cancel().into(),
            ConsensusAction::SetPage(page) => self.set_page(page).into(),
            ConsensusAction::SelectCandidate(i) => Task::future(select_format_folder(
                self.file_path.parent().unwrap_or(Path::new("")).into(),
            ))
            .and_then(move |(name, pages)| {
                Task::done(ConsensusAction::SetCandidate { i, name, pages })
            }),
            ConsensusAction::SetCandidate { i, name, pages } => {
                self.set_candidate(i, name, pages).into()
            }
            ConsensusAction::CleanText { page, part } => self.clean_text(page, part).into(),
            ConsensusAction::DropCandidate(i) => self.drop_candidate(i).into(),
        }
    }

    pub fn consensus(&mut self, page: usize) -> Result<Task<ConsensusAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };
        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.clear();
        }

        let Some(pages) = self.pages.get(..page + 1) else {
            let file_name = self.file_name();
            self.server.abort();
            return Ok(
                Task::future(complete_dialog(file_name.clone())).then(move |x| match x {
                    true => Task::done(ConsensusAction::SaveTranslation(file_name.to_owned())),
                    false => Task::none(),
                }),
            );
        };

        let candidates = candidates_map(&self.candidates, page);

        let task = self.server.consensus(pages, candidates, &model, page)?;

        Ok(task
            .chain(
                self.server
                    .bind_handle(Task::done(ConsensusAction::PageComplete(page))),
            )
            .chain(
                self.server
                    .bind_handle(Task::done(ConsensusAction::Consensus(page + 1))),
            ))
    }

    pub fn consensus_page(&mut self, page: usize) -> Result<Task<ConsensusAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.clear();
        }

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, page);

        let task = self.server.consensus(pages, candidates, &model, page)?;
        Ok(task
            .chain(
                self.server
                    .bind_handle(Task::done(ConsensusAction::PageComplete(page))),
            )
            .chain(Task::done(ServerAction::Abort.into())))
    }

    pub fn consensus_part(&mut self, page: usize, part: usize) -> Result<Task<ConsensusAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.sections.get_mut(part).unwrap().content.clear();
            page.jap_error.clear();
            page.size_error.clear();
        }

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, page);

        let task = self
            .server
            .consensus_part(pages, candidates, model, page, part)?;

        Ok(task
            .chain(
                self.server
                    .bind_handle(Task::done(ConsensusAction::PageComplete(page))),
            )
            .chain(Task::done(ServerAction::Abort.into())))
    }

    fn set_page(&mut self, page: usize) {
        self.current_page = page
    }

    fn cancel(&mut self) {
        self.pages
            .iter_mut()
            .filter(|p| matches!(p.activity, Activity::Active))
            .for_each(|p| p.activity = Activity::Incomplete);
        self.server.abort();
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_path = path;
        self.pages = pages;
    }

    fn check_complete(&mut self, page: usize) {
        let Some(page) = self.pages.get_mut(page) else {
            return;
        };

        page.size_error = page.check_size();
        page.jap_error = page.check_japanese();

        page.activity = if page.check_incomplete() {
            Activity::Incomplete
        } else if let Some(i) = page.size_error.first() {
            Activity::Error(i + 1)
        } else if let Some(i) = page.jap_error.first() {
            Activity::Error(i + 1)
        } else {
            Activity::Complete
        };
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content.push_str(&content);
            };
        };
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<ConsensusAction> {
        let tasks = self.pages.iter().map(|page| {
            let file_path = path
                .join(page.path.file_name().unwrap())
                .with_extension("md");

            let text: String = page
                .sections
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}{}\n", part_tag(i + 1), s.content))
                .collect();
            let contents = remove_think_tags(&text);

            Task::future(fs::write(file_path, contents)).then(|r| match r {
                Ok(()) => Task::none(),
                Err(e) => Task::future(display_error(e)),
            })
        });

        Task::batch(tasks).discard()
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<ConsensusAction> {
        match self.pages.get(page) {
            Some(page) => {
                let content: String = page
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}{}\n", part_tag(i + 1), e.content))
                    .collect();

                let name = format!("{name}.md");

                Task::future(save_file(name, content)).discard()
            }
            None => Task::none(),
        }
    }

    fn clean_text(&mut self, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content = clean_invisible_chars(&section.content)
            }
        };
    }

    fn set_candidate(&mut self, i: Option<usize>, name: String, pages: Vec<(PathBuf, String)>) {
        let re = Regex::new(r"<part>\d+</part>").unwrap();
        let pages = pages
            .into_iter()
            .map(|(path, text)| {
                let parts = re
                    .split(&text)
                    .filter(|e| !e.is_empty())
                    .map(str::to_owned)
                    .collect();
                (path, parts)
            })
            .collect();

        if let Some(i) = i {
            *self.candidates.get_mut(i).unwrap() = Candidate { name, pages };
        } else {
            self.candidates.push(Candidate { name, pages });
        };
    }

    fn drop_candidate(&mut self, i: usize) {
        self.candidates.remove(i);
    }
}

pub fn candidates_map(candidates: &Vec<Candidate>, page: usize) -> HashMap<&OsStr, Vec<&[String]>> {
    let candidates = candidates.iter().map(|e| &e.pages[..page + 1]).flatten();

    candidates.fold(HashMap::new(), |mut acc, (p, e)| {
        let name = p.file_stem().unwrap();
        acc.entry(name).or_insert_with(Vec::new).push(e);
        acc
    })
}

impl From<ServerAction> for ConsensusAction {
    fn from(action: ServerAction) -> Self {
        ConsensusAction::ServerAction(action)
    }
}
