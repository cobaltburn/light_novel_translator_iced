use crate::{
    actions::server_action::ServerAction,
    controller::{
        check_english_chars, get_ordered_path,
        parse::{partition_text, remove_think_tags},
        part_tag,
        xml_converter::{HEAD, XmlConverter},
    },
    error::{Error, Result},
    message::{display_error, select_epub},
    model::{
        Activity,
        translation::{Page, Translation},
    },
};
use epub::doc::EpubDoc;
use iced::Task;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
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
        name: String,
        pages: Vec<Page>,
    },
    Translate(usize),
    TranslatePage(usize),
    TranslatePart(usize, usize),
    CancelTranslate,
    SaveTranslation(String),
    ServerAction(ServerAction),
}

impl Translation {
    pub fn perform(&mut self, action: TransAction) -> Task<TransAction> {
        match action {
            TransAction::ServerAction(action) => self.server_state.perform(action).map(Into::into),
            TransAction::SetPage(page) => self.set_current_page(page).into(),
            TransAction::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            TransAction::PageComplete(page) => self.check_complete(page).into(),
            TransAction::CancelTranslate => self.cancel().into(),
            TransAction::SavePages(path) => self.save_pages(path),
            TransAction::SetEpub { name, pages } => self.set_epub(name, pages).into(),
            TransAction::SavePage { name, page } => self.save_page(name, page),
            TransAction::Translate(page) => match self.translate(page) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
            TransAction::TranslatePage(page) => match self.translate_page(page) {
                Ok(task) => task,
                Err(error) => Task::future(display_error(error)).discard(),
            },
            TransAction::TranslatePart(page, part) => match self.translate_part(page, part) {
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
            if let Some(part) = page.text.get_mut(part) {
                part.push_str(&content);
            };
        };
    }

    fn cancel(&mut self) {
        self.pages
            .iter_mut()
            .filter(|p| matches!(p.activity, Activity::Active))
            .for_each(|p| p.activity = Activity::Incomplete);
        self.server_state.abort();
    }

    fn set_current_page(&mut self, page: usize) {
        self.current_page = page;
    }

    fn check_complete(&mut self, page: usize) {
        let Some(page) = self.pages.get_mut(page) else {
            return;
        };

        if page.text.iter().any(|text| text.is_empty()) {
            page.activity = Activity::Incomplete;
        } else if page.text.iter().all(|text| check_english_chars(text, 75.0)) {
            page.activity = Activity::Complete;
        } else {
            page.activity = Activity::Error;
        }
    }

    pub fn save_page(&mut self, name: String, page: usize) -> Task<TransAction> {
        match self.pages.get(page) {
            Some(page) => {
                let content: String = page
                    .text
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{}{}\n", part_tag(i + 1), t))
                    .collect();

                let name = format!("{}.md", name);

                Task::future(save_file(name, content)).discard()
            }
            None => Task::none(),
        }
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<TransAction> {
        let tasks = self
            .pages
            .iter()
            .map(|page| {
                let name = page.path.file_stem().unwrap().to_os_string();
                let text: String = page
                    .text
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{}{}\n", part_tag(i + 1), t))
                    .collect();
                (name, remove_think_tags(&text))
            })
            .map(|(name, contents)| {
                let file_path = path.join(name).with_extension("md");
                Task::future(fs::write(file_path, contents)).then(|r| match r {
                    Ok(_) => Task::none(),
                    Err(error) => Task::future(display_error(error)),
                })
            });
        Task::batch(tasks).discard()
    }

    pub fn set_epub(&mut self, name: String, pages: Vec<Page>) {
        self.current_page = 0;
        self.file_name = name;
        self.pages = pages;
    }

    pub fn translate(&mut self, page: usize) -> Result<Task<TransAction>> {
        if !self.server_state.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server_state.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            let file_name = self.file_name.clone();
            return Ok(Task::done(ServerAction::Abort.into()).chain(
                Task::future(complete_dialog(file_name.clone())).then(move |x| match x {
                    true => Task::done(TransAction::SaveTranslation(file_name.clone())),
                    false => Task::none(),
                }),
            ));
        };

        current_page.activity = Activity::Active;

        current_page.clear_content();

        let tasks = self
            .server_state
            .translation_tasks(current_page, &model, page)?;
        let task = self.server_state.method.join_tasks(tasks);
        let complete_task = self
            .server_state
            .bind_handle(Task::done(TransAction::PageComplete(page)));
        let next_task = self
            .server_state
            .bind_handle(Task::done(TransAction::Translate(page + 1)));

        Ok(task.chain(complete_task).chain(next_task))
    }

    pub fn translate_page(&mut self, page: usize) -> Result<Task<TransAction>> {
        if !self.server_state.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server_state.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        current_page.activity = Activity::Active;

        current_page.clear_content();

        let tasks = self
            .server_state
            .translation_tasks(current_page, &model, page)?;
        let task = self.server_state.method.join_tasks(tasks);
        let complete_task = self
            .server_state
            .bind_handle(Task::done(TransAction::PageComplete(page)));

        let task = task
            .chain(complete_task)
            .chain(Task::done(ServerAction::Abort.into()));

        Ok(task)
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Result<Task<TransAction>> {
        if !self.server_state.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }

        let Some(model) = self.server_state.current_model.clone() else {
            return Err(Error::ServerError("No model selected"));
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Ok(Task::done(ServerAction::Abort.into()));
        };

        let Some(section) = current_page.sections.get_mut(part) else {
            log::error!("an invalid part was passed");
            return Ok(Task::none());
        };

        current_page.activity = Activity::Active;

        let text = current_page.text.get_mut(part).unwrap();
        text.clear();

        let task = self
            .server_state
            .translate_part(section.clone(), model, page, part)?;
        let complete_task = self
            .server_state
            .bind_handle(Task::done(TransAction::PageComplete(page)));

        let task = task
            .chain(complete_task)
            .chain(Task::done(ServerAction::Abort.into()));

        Ok(task)
    }
}

impl From<ServerAction> for TransAction {
    fn from(action: ServerAction) -> Self {
        TransAction::ServerAction(action)
    }
}

pub async fn load_folder_markdown() -> Option<Vec<(PathBuf, String)>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("load folder")
        .pick_folder()
        .await?;
    let mut dirs = fs::read_dir(handle.path()).await.ok()?;
    let mut pages = Vec::new();
    while let Ok(Some(entry)) = dirs.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|x| x == "md") {
            let content = fs::read_to_string(&path).await.ok()?;
            pages.push((path, content));
        }
    }

    Some(pages)
}

pub async fn pick_save_folder(file_name: String) -> Option<PathBuf> {
    let file_name = Path::new(&file_name).file_stem()?.to_str()?;
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn save_file(file_name: String, content: String) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(content.as_bytes()).await?
    }
    Ok(())
}

pub async fn get_pages(file_name: PathBuf, buffer: Vec<u8>) -> Result<(String, Vec<Page>)> {
    let mut epub = EpubDoc::from_reader(Cursor::new(buffer))?;

    let paths = get_ordered_path(&epub);
    let converter = XmlConverter {
        skip: vec![HEAD, b"image", b"img"],
    };

    let pages: Result<Vec<Page>> = paths
        .into_iter()
        .map(|path| {
            let html = epub.get_resource_str_by_path(&path).unwrap();
            (path, html)
        })
        .map(|(path, html)| {
            let markdown = converter.convert(&html)?;
            let mut sections = Vec::new();
            if !markdown.is_empty() {
                let partitioned = partition_text(&markdown);
                sections = partitioned.chunks(3).map(|x| x.join(" ")).collect();
            }
            Ok(Page::new(path, sections))
        })
        .collect();
    let pages: Vec<_> = pages?
        .into_iter()
        .filter(|p| !p.sections.is_empty())
        .collect();

    let file_name = file_name
        .file_name()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok((file_name, pages))
}

pub async fn complete_dialog(file_name: String) -> bool {
    let file_name = Path::new(&file_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    let dialog = rfd::AsyncMessageDialog::new()
        .set_title("Translation Complete")
        .set_description(format!("Save: {}", file_name))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        .await;

    matches!(dialog, rfd::MessageDialogResult::Yes)
}
