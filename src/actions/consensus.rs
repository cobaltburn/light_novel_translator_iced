use crate::{
    actions::{
        complete_dialog, get_pages, page_markdown, pick_save_folder, save_file, save_pages,
        select_format_folder, server,
    },
    controller::StreamAction,
    error::{Result, ResultTaskExt as _, TaskResultExt},
    message::{error_dialog, select_epub},
    model::{Activity, Candidate, Consensus, Page, check_page_at},
    view::{DisplayType, scroll_to_section},
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
    Cancel,
    Failed(String),
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
    ScrollToSection(usize),
    UpdateActivity(usize, Activity),
}

impl Consensus {
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::ServerAction(action) => self.server.perform(action).map(Into::into),
            Action::Consensus(page) => self.consensus(page).ok_or_display(),
            Action::ConsensusPage(page) => self.consensus_page(page).ok_or_display(),
            Action::ConsensusPart { page, part } => self.consensus_part(page, part).ok_or_display(),
            Action::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .ok_or_display(|path| Task::done(Action::SavePages(path))),
            Action::SavePages(path) => save_pages(&self.pages, &path),
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
            Action::Cancel => self.cancel().into(),
            Action::Failed(message) => self.fail(message),
            Action::SetPage(page) => self.set_page(page).into(),
            Action::SelectCandidate(i) => Task::future(select_format_folder(
                self.file_path
                    .as_ref()
                    .and_then(|path| Some(path.parent()?.into()))
                    .unwrap_or_default(),
            ))
            .and_then(move |(name, pages)| Task::done(Action::SetCandidate { i, name, pages })),
            Action::SetCandidate { i, name, pages } => self.set_candidate(i, name, pages).into(),
            Action::CleanText { page, part } => self.clean_text(page, part).into(),
            Action::DropCandidate(i) => self.drop_candidate(i).into(),
            Action::SetDisplay(display) => self.set_display(display).into(),
            Action::ScrollToSection(section) => scroll_to_section(section),
            Action::UpdateActivity(page, activity) => self.update_activity(page, activity).into(),
        }
    }

    fn set_display(&mut self, display: DisplayType) {
        self.display = display;
    }

    pub fn consensus(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;

        let Some(pages) = self.pages.get(..page + 1) else {
            let file_name = self.file_name().unwrap_or_default();
            self.server.abort();
            return Ok(Task::future(complete_dialog(file_name)).discard());
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self
            .server
            .consensus(pages, candidates, &model, page)
            .inspect_err(|_| self.cancel())?;
        self.pages[page].clear();

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
        let model = self.server.check_ready()?;

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self.server.consensus(pages, candidates, &model, page)?;
        self.pages[page].clear();

        let complete_task = self.complete_task(page);

        Ok(task
            .chain(complete_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    pub fn consensus_part(&mut self, page: usize, part: usize) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;
        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let candidates = candidates_map(&self.candidates, pages);

        let task = self
            .server
            .consensus_part(pages, candidates, &model, page, part)?;
        let current = &mut self.pages[page];
        current.sections[part].content.clear();
        current.errors.clear();

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
            .filter(|p| matches!(p.activity, Activity::Active(_)))
            .for_each(|p| p.activity = Activity::Incomplete);
        self.server.abort();
    }

    /// Stops the run and shows `message`. Streams that fail together each land
    /// here, so only the first one, which finds the run still active, reports.
    fn fail(&mut self, message: String) -> Task<Action> {
        if self.server.handles.is_empty() {
            return Task::none();
        }
        self.cancel();
        Task::future(error_dialog(message)).discard()
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_path = Some(path);
        self.pages = pages;
    }

    fn check_complete(&mut self, page: usize) {
        check_page_at(&mut self.pages, page);
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.content.push_str(&content);
        };
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<Action> {
        self.pages.get(page).map_or_default(|page| {
            Task::future(save_file(format!("{name}.md"), page_markdown(page)))
                .ok_or_display(Into::into)
        })
    }

    fn clean_text(&mut self, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.clean();
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

    fn update_activity(&mut self, page: usize, activity: Activity) {
        if let Some(page) = self.pages.get_mut(page) {
            page.activity = activity;
        }
    }
}

/// Groups candidate translations by file prefix, keeping only the pages that
/// match `pages` (matched by prefix, since candidate folders are unordered and
/// may contain fewer pages than the epub).
pub fn candidates_map<'a>(
    candidates: &'a [Candidate],
    pages: &[Page],
) -> HashMap<&'a OsStr, Vec<&'a [String]>> {
    let prefixes: HashSet<_> = pages.iter().filter_map(Page::file_prefix).collect();

    candidates
        .iter()
        .flat_map(|c| &c.pages)
        .filter_map(|(p, e)| Some((p.file_prefix()?, e)))
        .filter(|(name, _)| prefixes.contains(name))
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

impl StreamAction for Action {
    fn activity(page: usize, part: usize) -> Self {
        Action::UpdateActivity(page, Activity::Active(part + 1))
    }

    fn update(content: String, page: usize, part: usize) -> Self {
        Action::UpdateContent {
            content,
            page,
            part,
        }
    }

    fn fail(message: String) -> Self {
        Action::Failed(message)
    }

    fn clean(page: usize, part: usize) -> Self {
        Action::CleanText { page, part }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_of_several_failures_reports() {
        let mut model = Consensus::default();
        _ = model.server.bind_handle(Task::<()>::none());

        let first = model.perform(Action::Failed("offline".to_string()));
        let second = model.perform(Action::Failed("offline".to_string()));

        assert!(model.server.handles.is_empty());
        assert!(first.units() > 0);
        assert_eq!(second.units(), 0);
    }
}
