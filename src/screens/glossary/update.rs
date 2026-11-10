use crate::{
    chapter::{Activity, Chapter},
    dialogs::{error_dialog, save_file, select_epub},
    epub::get_chapters,
    error::{Error, Result, ResultTaskExt, TaskResultExt},
    llm::glossary_prompt,
    screens::glossary::Glossary,
    server,
    text::remove_think_tags,
};
use iced::Task;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Action {
    Server(server::Action),
    UpdateContent(usize, String),
    MarkComplete(usize),
    SetEpub {
        path: PathBuf,
        chapters: Vec<Chapter>,
    },
    SetPage(usize),
    Failed(String),
    OpenEpub,
    Glossary,
    NextPart,
    Cancel,
    Save,
}

impl Glossary {
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::Server(action) => self.server.perform(action).map(Into::into),
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(path, buf)| Task::future(get_chapters(path, buf)))
                .ok_or_display(|(path, chapters)| Task::done(Action::SetEpub { path, chapters })),
            Action::SetEpub { path, chapters } => self.set_epub(path, chapters).into(),
            Action::UpdateContent(chapter, content) => self.update_content(chapter, content).into(),
            Action::Glossary => self.glossary().ok_or_display(),
            Action::NextPart => self.advance().ok_or_display(),
            Action::Cancel => self.stop().into(),
            Action::Failed(message) => self.fail(message),
            Action::Save => self.save_content().ok_or_display(Into::into),
            Action::SetPage(page) => self.set_page(page).into(),
            Action::MarkComplete(page) => self.mark_complete(page).into(),
        }
    }

    pub fn set_epub(&mut self, path: PathBuf, chapters: Vec<Chapter>) {
        self.stop();
        self.file_path = Some(path);
        self.chapters = chapters;
    }

    pub fn set_page(&mut self, page: usize) {
        self.current_page = page;
    }

    pub fn update_content(&mut self, chapter: usize, content: String) {
        if let Some(chapter) = self.chapters.get_mut(chapter) {
            chapter.content.push_str(&content);
        }
    }

    pub fn glossary(&mut self) -> Result<Task<Action>> {
        self.server.check_ready()?;
        self.pending = (self.current_page..self.chapters.len()).collect();
        if self.pending.is_empty() {
            return Err(Error::Error("No text to read in this epub"));
        }
        self.advance()
    }

    fn advance(&mut self) -> Result<Task<Action>> {
        self.next_part().inspect_err(|_| self.stop())
    }

    /// Keeps the pass that just ended, then sends the next part of the book
    /// along with it.
    fn next_part(&mut self) -> Result<Task<Action>> {
        let Some(chapter) = self.pending.pop_front() else {
            self.stop();
            return Ok(Task::none());
        };

        let (prior, current) = match self.chapters.split_at(chapter) {
            ([.., a], [b, ..]) => (Some(a), b),
            ([], [b, ..]) => (None, b),
            _ => panic!("how we get here"),
        };

        let existing = prior.map(|e| e.content.as_str());
        let book_text = &current.text;

        let task = self
            .server
            .glossary(chapter, glossary_prompt(existing, book_text))?;
        self.chapters[chapter].content.clear();

        let next = self.server.bind_handle(Task::done(Action::NextPart));
        let complete_task = self.complete_task(chapter);
        self.chapters[chapter].activity = Activity::Active;
        Ok(task.chain(complete_task).chain(next))
    }

    fn complete_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::MarkComplete(page)))
    }

    /// Ends the run. Safe to call when idle. A pass that was cut short gives
    /// way to the last complete glossary, because each pass rewrites it whole.
    fn stop(&mut self) {
        let chapter = self
            .chapters
            .iter_mut()
            .find(|c| matches!(c.activity, Activity::Active));
        if let Some(chapter) = chapter {
            chapter.activity = Activity::Incomplete;
        }
        self.pending.clear();
        self.server.abort();
    }

    /// Stops the run and shows `message`.
    fn fail(&mut self, message: String) -> Task<Action> {
        self.stop();
        Task::future(error_dialog(message)).discard()
    }

    fn save_content(&self) -> Task<Result<()>> {
        if let Some(file_stem) = self.file_stem()
            && let Some(chapter) = self.chapters.iter().rfind(|c| !c.content.is_empty())
        {
            let content = remove_think_tags(&chapter.content);
            return Task::future(save_file(format!("{file_stem}.md"), content));
        };
        Task::none()
    }

    fn mark_complete(&mut self, page: usize) {
        if let Some(chapter) = self.chapters.get_mut(page) {
            chapter.activity = Activity::Complete;
        }
    }
}

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::Server(action)
    }
}
