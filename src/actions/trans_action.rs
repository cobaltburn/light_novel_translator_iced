use crate::{
    actions::{
        clean_invisible_chars, complete_dialog, get_pages, pick_save_folder, save_file,
        server_action::ServerAction,
    },
    controller::{parse::remove_think_tags, part_tag},
    error::{Error, Result},
    message::{display_error, select_epub},
    model::{Activity, page::Page, translation::Translation},
};
use iced::Task;
use std::path::PathBuf;
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
            TransAction::SetPage(page) => self.set_page(page).into(),
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
            TransAction::Translate(page) => match self.translate(page) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
            TransAction::TranslatePage(page) => match self.translate_page(page) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
            TransAction::TranslatePart { page, part } => match self.translate_part(page, part) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
            TransAction::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .then(|doc| match doc {
                    Ok((name, pages)) => Task::done(TransAction::SetEpub { name, pages }.into()),
                    Err(error) => Task::future(display_error(error)).discard(),
                }),
            TransAction::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .then(|path| match path {
                    Ok(path) => Task::done(TransAction::SavePages(path).into()),
                    Err(err) => Task::future(display_error(err)).discard(),
                }),
        }
    }

    pub fn update_content(&mut self, content: String, page: usize, part: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            if let Some(section) = page.sections.get_mut(part) {
                section.content.push_str(&content);
            };
        };
    }

    fn cancel(&mut self) -> Task<TransAction> {
        let page = self
            .pages
            .iter_mut()
            .position(|p| matches!(p.activity, Activity::Active));

        self.server.abort();
        if let Some(page) = page {
            Task::done(TransAction::PageComplete(page))
        } else {
            Task::none()
        }
    }

    fn set_page(&mut self, page: usize) {
        self.current_page = page
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
                Ok(()) => Task::none(),
                Err(e) => Task::future(display_error(e)),
            })
        });

        Task::batch(tasks).discard()
    }

    pub fn set_epub(&mut self, path: PathBuf, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_path = path;
        self.pages = pages;
    }

    pub fn translate(&mut self, page: usize) -> Result<Task<TransAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            let file_name = self.file_name();
            self.server.abort();
            return Ok(
                Task::future(complete_dialog(file_name.clone())).then(move |x| match x {
                    true => Task::done(TransAction::SaveTranslation(file_name.to_owned())),
                    false => Task::none(),
                }),
            );
        };

        let current_page = pages.last_mut().unwrap();
        current_page.activity = Activity::Active;
        current_page.clear();

        let task = self.server.translate(pages, &model, page)?;

        let complete_task = self
            .server
            .bind_handle(Task::done(TransAction::PageComplete(page)));
        let next_task = self
            .server
            .bind_handle(Task::done(TransAction::Translate(page + 1)));

        Ok(task.chain(complete_task).chain(next_task))
    }

    pub fn translate_page(&mut self, page: usize) -> Result<Task<TransAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(pages) = self.pages.get_mut(0..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let current_page = pages.last_mut().unwrap();
        current_page.activity = Activity::Active;
        current_page.clear();

        let task = self.server.translate(pages, &model, page)?;
        let complete_task = self
            .server
            .bind_handle(Task::done(TransAction::PageComplete(page)));

        let task = task
            .chain(complete_task)
            .chain(Task::done(ServerAction::Abort.into()));

        Ok(task)
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Result<Task<TransAction>> {
        if !self.server.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(pages) = self.pages.get_mut(..page + 1) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let current = pages.last_mut().unwrap();
        current.activity = Activity::Active;
        current.sections.get_mut(part).unwrap().content.clear();

        let task = self.server.translate_part(pages, model, page, part)?;
        let complete_task = self
            .server
            .bind_handle(Task::done(TransAction::PageComplete(page)));

        let task = task
            .chain(complete_task)
            .chain(Task::done(ServerAction::Abort.into()));

        Ok(task)
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
