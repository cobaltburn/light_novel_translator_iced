use crate::{
    actions::{
        clean_invisible_chars, complete_dialog, get_pages, pick_save_folder, save_file,
        select_format_folder, server,
    },
    controller::{part_tag, remove_think_tags},
    error::{Error, Result, ResultTaskExt as _, TaskResultExt},
    message::{display_error, select_epub},
    model::{Activity, Candidate, Consensus, Page},
    view::DisplayType,
};
use iced::Task;
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::PathBuf,
    sync::LazyLock,
};
use tokio::fs;

static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<part>\d+</part>").unwrap());

#[derive(Debug, Clone)]
pub enum Action {
    ServerAction(server::Action),
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
    SetDisplay(DisplayType),
}

impl Consensus {
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::ServerAction(action) => self.server.perform(action).map(Into::into),
            Action::Consensus(page) => self.consensus(page).ok_or_display(),
            Action::ConsensusPage(page) => self.consensus_page(page).ok_or_display(),
            Action::ConsensusPart { page, part } => self.consensus_part(page, part).ok_or_display(),
            Action::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .map_err(Error::from)
                .ok_or_display(|path| Task::done(Action::SavePages(path))),
            Action::SavePages(path) => self.save_pages(path),
            Action::SavePage { name, page } => self.save_page(name, page),
            Action::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            Action::PageComplete(page) => self.check_complete(page).into(),
            Action::SetEpub { path: name, pages } => self.set_epub(name, pages).into(),
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .ok_or_display(|(path, pages)| Task::done(Action::SetEpub { path, pages })),
            Action::CancelConsensus => self.cancel().into(),
            Action::SetPage(page) => self.set_page(page).into(),
            Action::SelectCandidate(i) => Task::future(select_format_folder(
                self.file_path.parent().map_or(PathBuf::new(), Into::into),
            ))
            .and_then(move |(name, pages)| Task::done(Action::SetCandidate { i, name, pages })),
            Action::SetCandidate { i, name, pages } => self.set_candidate(i, name, pages).into(),
            Action::CleanText { page, part } => self.clean_text(page, part).into(),
            Action::DropCandidate(i) => self.drop_candidate(i).into(),
            Action::SetDisplay(display) => self.set_display(display).into(),
        }
    }

    fn set_display(&mut self, display: DisplayType) {
        self.display = display;
    }

    fn check_ready(&self) -> Result<String> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }
        self.server
            .current_model
            .clone()
            .ok_or(Error::ServerError("No model selected"))
    }

    pub fn consensus(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;
        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.clear();
        }

        let Some(pages) = self.pages.get(..page + 1) else {
            let file_name = self.file_name();
            self.server.abort();
            return Ok(Task::future(complete_dialog(file_name.clone())).discard());
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self.server.consensus(pages, candidates, &model, page)?;
        let complete_task = self.complete_task(page);
        let next_task = self.next_task(page);

        Ok(task.chain(complete_task).chain(next_task))
    }

    fn complete_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::PageComplete(page)))
    }

    fn next_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::Consensus(page + 1)))
    }

    pub fn consensus_page(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;

        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.clear();
        }

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self.server.consensus(pages, candidates, &model, page)?;
        let complete_task = self.complete_task(page);

        Ok(task
            .chain(complete_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    pub fn consensus_part(&mut self, page: usize, part: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;

        if let Some(page) = self.pages.get_mut(page) {
            page.activity = Activity::Active;
            page.sections.get_mut(part).unwrap().content.clear();
            page.errors.clear();
        }

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self
            .server
            .consensus_part(pages, candidates, model, page, part)?;
        let complete_task = self
            .server
            .bind_handle(Task::done(Action::PageComplete(page)));

        Ok(task
            .chain(complete_task)
            .chain(Task::done(server::Action::Abort.into())))
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
        let last_section = self
            .pages
            .get(page - 1)
            .and_then(|p| Some(p.sections.last()?.content.clone()))
            .unwrap_or_default();
        if let Some(page) = self.pages.get_mut(page) {
            page.check_page(&last_section);
        };
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.content.push_str(&content);
        };
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<Action> {
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

    pub fn save_page(&mut self, name: String, page: usize) -> Task<Action> {
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
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.content = clean_invisible_chars(&section.content).replace(['“', '”'], "\"");
        };
    }

    fn set_candidate(&mut self, i: Option<usize>, name: String, pages: Vec<(PathBuf, String)>) {
        let pages = pages
            .into_iter()
            .map(|(path, text)| {
                let parts = PART_RE
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

/// Groups candidate translations by file stem, keeping only the pages that
/// match `pages` (matched by stem, since candidate folders are unordered and
/// may contain fewer pages than the epub).
pub fn candidates_map<'a>(
    candidates: &'a [Candidate],
    pages: &[Page],
) -> HashMap<&'a OsStr, Vec<&'a [String]>> {
    let stems: HashSet<_> = pages.iter().filter_map(|p| p.path.file_stem()).collect();

    candidates
        .iter()
        .flat_map(|c| &c.pages)
        .filter_map(|(p, e)| Some((p.file_stem()?, e)))
        .filter(|(name, _)| stems.contains(name))
        .fold(HashMap::new(), |mut acc, (name, e)| {
            acc.entry(name).or_insert_with(Vec::new).push(e.as_slice());
            acc
        })
}

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::ServerAction(action)
    }
}
