use crate::{
    actions::{complete_dialog, get_pages, load_recovery, pick_save_folder, save_file, server},
    controller::{part_tag, remove_think_tags},
    error::{Error, Result, ResultTaskExt as _, TaskResultExt},
    message::select_epub,
    model::{Activity, Page, Translation},
    view::DisplayType,
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
    Recover,
    RecoverPages(Vec<Page>),
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
    CancelTranslate,
    SaveTranslation(String),
    ServerAction(server::Action),
    SetDisplay(DisplayType),
}

impl Translation {
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::ServerAction(action) => self.server.perform(action).map(Into::into),
            Action::SetPage(page) => self.set_current_page(page).into(),
            Action::CleanText { page, part } => self.clean_text(page, part).into(),
            Action::PageComplete(page) => self.check_complete(page).into(),
            Action::CancelTranslate => self.cancel(),
            Action::SavePages(path) => self.save_pages(path),
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
            Action::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .map_err(Error::from)
                .ok_or_display(|path| Task::done(Action::SavePages(path))),
            Action::RecoverPages(pages) => self.recover_pages(pages).ok_or_display(),
            Action::Recover => Task::future(load_recovery())
                .and_then(|pages| Task::done(Action::RecoverPages(pages))),
            Action::SetDisplay(display) => self.set_display(display).into(),
        }
    }

    fn set_display(&mut self, display: DisplayType) {
        self.display = display;
    }

    pub fn recover_pages(&mut self, pages: Vec<Page>) -> Result<()> {
        let mut sections: HashMap<_, _> = pages.into_iter().map(|p| (p.path, p.sections)).collect();

        let mut last_section = "";
        for page in self.pages.iter_mut() {
            if let Some(current) = sections.get_mut(&page.path) {
                mem::swap(&mut page.sections, current);
                page.check_page(last_section);
            }
            last_section = page.sections.last().map_or_default(|s| s.content.as_str());
        }

        self.pages
            .iter_mut()
            .for_each(|p| p.sections.iter_mut().for_each(|s| s.clean()));

        Ok(())
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
        Task::future(async move { fs::write(path, contents?).await })
            .map_err(Error::from)
            .ok_or_display(Into::into)
    }

    fn cancel(&mut self) -> Task<Action> {
        let page = self
            .pages
            .iter_mut()
            .position(|p| matches!(p.activity, Activity::Active));

        self.server.abort();
        page.map_or_default(|page| Task::done(Action::PageComplete(page)))
    }

    fn set_current_page(&mut self, page: usize) {
        self.current_page = page
    }

    fn check_complete(&mut self, page: usize) {
        let last_section = page
            .checked_sub(1)
            .and_then(|i| self.pages.get(i))
            .and_then(|p| p.sections.last())
            .map_or_default(|c| c.content.clone());

        if let Some(page) = self.pages.get_mut(page) {
            page.check_page(&last_section);
        };
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<Action> {
        self.pages.get(page).map_or_default(|page| {
            let text: String = page
                .sections
                .iter()
                .enumerate()
                .map(|(i, e)| format!("{}{}\n", part_tag(i + 1), e.content))
                .collect();
            let contents = remove_think_tags(&text);

            Task::future(save_file(format!("{name}.md"), contents)).discard()
        })
    }

    pub fn save_pages(&self, path: PathBuf) -> Task<Action> {
        let pages: Vec<_> = self
            .pages
            .iter()
            .map(|page| {
                let file_path = path
                    .join(page.path.file_name().unwrap())
                    .with_extension("md");

                let text: String = page
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("{}{}\n", part_tag(i + 1), s.content))
                    .collect();
                (file_path, remove_think_tags(&text))
            })
            .collect();

        Task::future(async move {
            for (path, contents) in pages {
                fs::write(path, contents).await?
            }
            Ok(())
        })
        .ok_or_display(Into::into)
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_path = path;
        self.pages = pages;
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

    pub fn translate(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            let file_name = self.file_name();
            self.server.abort();
            return Ok(Task::future(complete_dialog(file_name)).discard());
        };

        let current_page = pages.last_mut().unwrap();
        current_page.activity = Activity::Active;
        current_page.clear();

        let task = self.server.translate(pages, &model, page)?;

        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();
        let next_task = self.next_task(page);

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(next_task))
    }

    fn complete_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::PageComplete(page)))
    }

    fn backup_task(&mut self) -> Task<Action> {
        let backup = self.file_path.with_extension("json");

        self.server
            .bind_handle(Task::done(Action::SaveRecovery(backup)))
    }

    fn next_task(&mut self, page: usize) -> Task<Action> {
        self.server
            .bind_handle(Task::done(Action::Translate(page + 1)))
    }

    pub fn translate_page(&mut self, page: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(0..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let current_page = pages.last_mut().unwrap();
        current_page.activity = Activity::Active;
        current_page.clear();

        let task = self.server.translate(pages, &model, page)?;
        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Result<Task<Action>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            return Ok(Task::done(server::Action::Abort.into()));
        };

        let current = pages.last_mut().unwrap();
        current.activity = Activity::Active;
        current.sections.get_mut(part).unwrap().content.clear();
        current.errors.clear();

        let task = self.server.translate_part(pages, &model, page, part)?;
        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(Task::done(server::Action::Abort.into())))
    }

    fn clean_text(&mut self, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page)
            && let Some(section) = page.sections.get_mut(part)
        {
            section.clean();
        };
    }
}

impl From<server::Action> for Action {
    fn from(action: server::Action) -> Self {
        Action::ServerAction(action)
    }
}
