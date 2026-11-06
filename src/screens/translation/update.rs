use crate::{
    dialogs::{
        complete_dialog, error_dialog, load_recovery, pick_save_folder, save_file, select_epub,
        select_glossary,
    },
    epub::get_pages,
    error::{Result, ResultTaskExt as _, TaskResultExt},
    llm::StreamAction,
    page::{
        Activity, DisplayType, Page, absolute_page_path, check_all_pages, check_page_at,
        page_markdown, save_pages,
    },
    screens::translation::Translation,
    server,
    ui::scroll_to_section,
};
use iced::Task;
use std::{collections::HashMap, mem, path::PathBuf};
use tokio::fs;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Action {
    SetPage(usize),
    UpdateContent {
        content: String,
        page: usize,
        part: usize,
    },
    PageComplete(usize),
    SavePages(PathBuf),
    SavePage {
        name: String,
        page: usize,
    },
    SaveRecovery(PathBuf),
    RecoverPages(Vec<Page>),
    Recover,
    OpenGlossary,
    SetGlossary(PathBuf, Vec<u8>),
    OpenEpub,
    SetEpub {
        name: PathBuf,
        pages: Vec<Page>,
    },
    Translate(usize),
    TranslatePage(usize),
    TranslatePart {
        page: usize,
        part: usize,
    },
    CleanText {
        page: usize,
        part: usize,
    },
    Cancel,
    Failed(String),
    Save(String),
    Server(server::Action),
    SetDisplay(DisplayType),
    ScrollToSection(usize),
    UpdateActivity(usize, Activity),
}

impl Translation {
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::Server(action) => self.server.perform(action).map(Into::into),
            Action::SetPage(page) => self.set_current_page(page).into(),
            Action::CleanText { page, part } => self.clean_text(page, part).into(),
            Action::PageComplete(page) => self.check_complete(page).into(),
            Action::Cancel => self.cancel(),
            Action::Failed(message) => self.fail(message),
            Action::SavePages(path) => save_pages(&self.pages, &path),
            Action::SetEpub { name, pages } => self.set_epub(name, pages).into(),
            Action::SavePage { name, page } => self.save_page(name, page),
            Action::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            Action::Translate(page) => self.translate(page).ok_or_display(),
            Action::TranslatePage(page) => self.translate_page(page).ok_or_display(),
            Action::TranslatePart { page, part } => self.translate_part(page, part).ok_or_display(),
            Action::SaveRecovery(path) => self.save_json(path),
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .ok_or_display(|(name, pages)| Task::done(Action::SetEpub { name, pages })),
            Action::Save(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .ok_or_display(|path| Task::done(Action::SavePages(path))),
            Action::RecoverPages(pages) => self.recover_pages(pages).into(),
            Action::Recover => Task::future(load_recovery())
                .and_then(|pages| Task::done(Action::RecoverPages(pages))),
            Action::SetDisplay(display) => self.set_display(display).into(),
            Action::ScrollToSection(section) => scroll_to_section(section),
            Action::UpdateActivity(page, activity) => self.update_activity(page, activity).into(),
            Action::OpenGlossary => Task::future(select_glossary())
                .and_then(|(path, content)| Task::done(Action::SetGlossary(path, content))),
            Action::SetGlossary(path, content) => self.set_glossary(path, content).ok_or_display(),
        }
    }

    fn set_display(&mut self, display: DisplayType) {
        self.display = display;
    }

    pub fn recover_pages(&mut self, pages: Vec<Page>) -> () {
        let mut sections: HashMap<PathBuf, _> = pages
            .into_iter()
            .map(|p| (absolute_page_path(p.path), p.sections))
            .collect();

        for page in self.pages.iter_mut() {
            if let Some(current) = sections.get_mut(&page.path) {
                for (x, y) in page.sections.iter_mut().zip(current) {
                    mem::swap(&mut x.content, &mut y.content);
                }
            }
        }

        self.pages
            .iter_mut()
            .for_each(|p| p.sections.iter_mut().for_each(|s| s.clean()));

        check_all_pages(&mut self.pages);
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.content.push_str(&content);
        };
    }

    fn save_json(&self, path: PathBuf) -> Task<Action> {
        let contents = serde_json::to_string_pretty(&self.pages);
        Task::future(async move { fs::write(path, contents?).await }).ok_or_display(Into::into)
    }

    fn cancel(&mut self) -> Task<Action> {
        let page = self
            .pages
            .iter_mut()
            .position(|p| matches!(p.activity, Activity::Active(_)));

        self.server.abort();
        page.map_or_default(|page| Task::done(Action::PageComplete(page)))
    }

    /// Stops the run and shows `message`. Streams that fail together each land
    /// here, so only the first one, which finds the run still active, reports.
    fn fail(&mut self, message: String) -> Task<Action> {
        if self.server.handles.is_empty() {
            return Task::none();
        }
        self.cancel()
            .chain(Task::future(error_dialog(message)).discard())
    }

    fn set_current_page(&mut self, page: usize) {
        self.current_page = page
    }

    fn check_complete(&mut self, page: usize) {
        check_page_at(&mut self.pages, page);
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<Action> {
        self.pages.get(page).map_or_default(|page| {
            Task::future(save_file(format!("{name}.md"), page_markdown(page)))
                .ok_or_display(Into::into)
        })
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_path = Some(path);
        self.pages = pages;
        self.glossary = None;
    }

    fn set_glossary(&mut self, path: PathBuf, content: Vec<u8>) -> Result<()> {
        self.glossary = Some((path, String::from_utf8(content)?));
        Ok(())
    }

    pub fn translate(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;

        let Some(pages) = self.pages.get(..page + 1) else {
            let file_name = self.epub_name().unwrap_or_default().into_owned();
            self.server.abort();
            return Ok(Task::future(complete_dialog(file_name)).discard());
        };

        let glossary = self.glossary.as_ref().map(|e| e.1.as_str());
        let task = self.server.translate(pages, &model, page, glossary)?;
        self.pages[page].clear();

        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();
        let next_task = self.next_translate_task(page);

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(next_task))
    }

    pub fn translate_page(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;

        let Some(pages) = self.pages.get(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let glossary = self.glossary.as_ref().map(|e| e.1.as_str());
        let task = self.server.translate(pages, &model, page, glossary)?;
        self.pages[page].clear();

        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Result<Task<Action>> {
        let model = self.server.check_ready()?;

        let Some(pages) = self.pages.get(..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let glossary = self.glossary.as_ref().map(|e| e.1.as_str());
        let task = self
            .server
            .translate_part(pages, &model, page, part, glossary)?;
        let current = &mut self.pages[page];
        current.sections[part].content.clear();
        current.errors.clear();

        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    fn complete_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::PageComplete(page)))
    }

    fn backup_task(&mut self) -> Task<Action> {
        let Some(path) = self.file_path.as_ref() else {
            return Task::none();
        };

        self.server.bind_handle(Task::done(Action::SaveRecovery(
            path.with_extension("json"),
        )))
    }

    fn next_translate_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::Translate(page + 1)))
    }

    fn clean_text(&mut self, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.clean();
        };
    }
    fn update_activity(&mut self, page: usize, activity: Activity) {
        if let Some(page) = self.pages.get_mut(page) {
            page.activity = activity;
        }
    }
}

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::Server(action)
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
        let mut model = Translation::default();
        _ = model.server.bind_handle(Task::<()>::none());

        let first = model.perform(Action::Failed("offline".to_string()));
        let second = model.perform(Action::Failed("offline".to_string()));

        assert!(model.server.handles.is_empty());
        assert!(first.units() > 0);
        assert_eq!(second.units(), 0);
    }
}
