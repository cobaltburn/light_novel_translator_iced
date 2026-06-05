use crate::{
    actions::{
        clean_invisible_chars, complete_dialog, get_pages, load_recovery, pick_save_folder,
        save_file, server_action::ServerAction,
    },
    controller::{parse::remove_think_tags, part_tag},
    error::{Error, Result},
    message::{display_error, select_epub},
    model::{Activity, page::Page, translation::Translation},
};
use iced::Task;
use std::{collections::HashMap, mem, path::PathBuf};
use tokio::fs;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TransAction {
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
    ServerAction(ServerAction),
}

impl Translation {
    pub fn perform(&mut self, action: TransAction) -> Task<TransAction> {
        match action {
            TransAction::ServerAction(action) => self.server.perform(action).map(Into::into),
            TransAction::SetPage(page) => self.set_current_page(page).into(),
            TransAction::CleanText { page, part } => self.clean_text(page, part).into(),
            TransAction::PageComplete(page) => self.check_complete(page).into(),
            TransAction::CancelTranslate => self.cancel().into(),
            TransAction::SavePages(path) => self.save_pages(path),
            TransAction::SetEpub { name, pages } => self.set_epub(name, pages).into(),
            TransAction::SavePage { name, page } => self.save_page(name, page),
            TransAction::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            TransAction::Translate(page) => self
                .translate(page)
                .unwrap_or_else(|error| error.display_error()),
            TransAction::TranslatePage(page) => self
                .translate_page(page)
                .unwrap_or_else(|error| error.display_error()),
            TransAction::TranslatePart { page, part } => self
                .translate_part(page, part)
                .unwrap_or_else(|error| error.display_error()),
            TransAction::SaveRecovery(path) => self
                .save_json(path)
                .unwrap_or_else(|error| error.display_error()),
            TransAction::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .then(|doc| match doc {
                    Ok((name, pages)) => Task::done(TransAction::SetEpub { name, pages }),
                    Err(error) => error.display_error(),
                }),
            TransAction::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .then(|path| match path {
                    Ok(path) => Task::done(TransAction::SavePages(path).into()),
                    Err(err) => Task::future(display_error(err)).discard(),
                }),
            TransAction::RecoverPages(pages) => match self.recover_pages(pages) {
                Ok(_) => Task::none(),
                Err(err) => err.display_error(),
            },
            TransAction::Recover => Task::future(load_recovery())
                .and_then(|pages| Task::done(TransAction::RecoverPages(pages))),
        }
    }

    pub fn recover_pages(&mut self, pages: Vec<Page>) -> Result<()> {
        let mut sections: HashMap<_, _> = pages.into_iter().map(|p| (p.path, p.sections)).collect();

        let mut last_section = String::new();
        for page in self.pages.iter_mut() {
            if let Some(current) = sections.get_mut(&page.path) {
                mem::swap(&mut page.sections, current);
                page.check_page(&last_section);
            }
            last_section = page
                .sections
                .last()
                .map(|s| s.content.clone())
                .unwrap_or_default();
        }

        Ok(())
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content.push_str(&content);
            };
        };
    }

    fn save_json(&self, path: PathBuf) -> Result<Task<TransAction>> {
        let contents = serde_json::to_string_pretty(&self.pages)?;
        Ok(Task::future(fs::write(path, contents)).then(|e| match e {
            Err(error) => Error::from(error).display_error(),
            Ok(_) => Task::none(),
        }))
    }

    fn cancel(&mut self) -> Task<TransAction> {
        let page = self
            .pages
            .iter_mut()
            .position(|p| matches!(p.activity, Activity::Active));

        self.server.abort();
        page.map(|page| Task::done(TransAction::PageComplete(page)))
            .unwrap_or_default()
    }

    fn set_current_page(&mut self, page: usize) {
        self.current_page = page
    }

    fn check_complete(&mut self, page: usize) {
        let last_section = page
            .checked_sub(1)
            .and_then(|i| self.pages.get(i))
            .and_then(|p| p.sections.last())
            .map(|c| c.content.clone())
            .unwrap_or_default();

        if let Some(page) = self.pages.get_mut(page) {
            page.check_page(&last_section);
        };
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<TransAction> {
        match self.pages.get(page) {
            Some(page) => {
                let content: String = page
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}{}\n", part_tag(i + 1), e.content))
                    .collect();

                let name = format!("{}.md", name);

                Task::future(save_file(name, content)).discard()
            }
            None => Task::none(),
        }
    }

    pub fn save_pages(&self, path: PathBuf) -> Task<TransAction> {
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
                Ok(_) => Task::none(),
                Err(error) => Task::future(display_error(error)),
            })
        });

        Task::batch(tasks).discard()
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

    pub fn translate(&mut self, page: usize) -> Result<Task<TransAction>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            let file_name = self.file_name();
            self.server.abort();
            return Ok(Task::future(complete_dialog(file_name.clone())).discard());
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

    fn complete_task(&mut self, page: usize) -> Task<TransAction> {
        self.server
            .bind_handle(Task::done(TransAction::PageComplete(page)))
    }

    fn backup_task(&mut self) -> Task<TransAction> {
        let backup = self.file_path.with_extension("json");

        self.server
            .bind_handle(Task::done(TransAction::SaveRecovery(backup)))
    }

    fn next_task(&mut self, page: usize) -> Task<TransAction> {
        self.server
            .bind_handle(Task::done(TransAction::Translate(page + 1)))
    }

    pub fn translate_page(&mut self, page: usize) -> Result<Task<TransAction>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(0..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
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
            .chain(Task::done(ServerAction::Abort.into())))
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Result<Task<TransAction>> {
        let model = self.check_ready()?;

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let current = pages.last_mut().unwrap();
        current.activity = Activity::Active;
        current.sections.get_mut(part).unwrap().content.clear();
        current.errors.clear();

        let task = self.server.translate_part(pages, model, page, part)?;
        let complete_task = self.complete_task(page);
        let backup_task = self.backup_task();

        Ok(task
            .chain(complete_task)
            .chain(backup_task)
            .chain(Task::done(ServerAction::Abort.into())))
    }

    fn clean_text(&mut self, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content = clean_invisible_chars(&section.content)
            }
        };
    }
}

impl From<ServerAction> for TransAction {
    fn from(action: ServerAction) -> Self {
        TransAction::ServerAction(action)
    }
}
