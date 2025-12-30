use crate::{
    actions::server_action::ServerAction,
    controller::{
        doc::get_ordered_path,
        markdown::{convert_html, partition_text},
        xml::{part_tag, remove_think_tags},
    },
    error::{Error, Result},
    message::{display_error, open_epub},
    state::translation_model::{Method, Page, TranslationModel},
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
    SavePage(String, usize),
    OpenEpub,
    SetEpub((String, Vec<Page>)),
    Translate(usize),
    TranslatePage(usize),
    TranslatePart(usize, usize),
    SaveTranslation(String),
    ServerAction(ServerAction),
    SetMethod(Method),
}

impl TranslationModel {
    pub fn perform(&mut self, action: TransAction) -> Task<TransAction> {
        match action {
            TransAction::ServerAction(action) => self.server_state.perform(action),
            TransAction::SetPage(page) => self.set_current_page(page).into(),
            TransAction::UpdateContent {
                content,
                page,
                part,
            } => self.update_content(content, page, part).into(),
            TransAction::PageComplete(page) => self.mark_complete(page).into(),
            TransAction::Translate(page) => self.translate(page),
            TransAction::TranslatePage(page) => self.translate_page(page),
            TransAction::TranslatePart(page, part) => self.translate_part(page, part),
            TransAction::SavePages(path) => self.save_pages(path),
            TransAction::SetEpub((file_name, buffer)) => self.set_epub(file_name, buffer).into(),
            TransAction::SetMethod(method) => self.set_method(method).into(),
            TransAction::SavePage(file_name, page) => self.save_page(file_name, page),
            TransAction::OpenEpub => Task::future(open_epub())
                .and_then(|(name, buffer)| Task::future(get_pages(name, buffer)))
                .then(|doc| match doc {
                    Ok(doc) => Task::done(TransAction::SetEpub(doc).into()),
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

    pub fn set_method(&mut self, method: Method) {
        self.method = method;
    }

    pub fn set_current_page(&mut self, page: usize) {
        self.current_page = Some(page);
    }

    pub fn mark_complete(&mut self, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.complete = true;
        }
    }

    pub fn save_page(&mut self, file_name: String, page: usize) -> Task<TransAction> {
        match self.pages.get(page) {
            Some(page) => {
                let content = page.text.join(" ");
                Task::future(save_file(file_name, content)).discard()
            }
            None => Task::none(),
        }
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<TransAction> {
        let tasks = self
            .pages
            .iter()
            .filter(|page| page.complete && !page.text.is_empty())
            .map(|page| {
                let name = page.path.file_stem().unwrap().to_os_string();
                (name, remove_think_tags(&page.text.join("\n")))
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

    pub fn set_epub(&mut self, file_name: String, pages: Vec<Page>) {
        self.current_page = Some(0);
        self.file_name = file_name;
        self.pages = pages;
    }

    pub fn translate(&mut self, page: usize) -> Task<TransAction> {
        if !self.server_state.connected() {
            let error = Error::ServerError("Not connected to a server");
            return Task::future(display_error(error)).discard();
        }

        let Some(model) = self.server_state.current_model.clone() else {
            let error = Error::ServerError("No model selected");
            return Task::future(display_error(error)).discard();
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            let file_name = self.file_name.clone();
            return Task::done(ServerAction::Abort.into()).chain(
                Task::future(complete_dialog(file_name.clone())).then(move |x| match x {
                    true => Task::done(TransAction::SaveTranslation(file_name.clone())),
                    false => Task::none(),
                }),
            );
        };

        current_page.clear_content();

        let tasks = self.server_state.collect_task(current_page, &model, page);
        let task = self.method.join_tasks(tasks);
        let complete_task = self.bind_handle(Task::done(TransAction::PageComplete(page)));
        let next_task = self.bind_handle(Task::done(TransAction::Translate(page + 1)));

        task.chain(complete_task).chain(next_task)
    }

    pub fn translate_page(&mut self, page: usize) -> Task<TransAction> {
        if !self.server_state.connected() {
            let error = Error::ServerError("Not connected to a server");
            return Task::future(display_error(error)).discard();
        }

        let Some(model) = self.server_state.current_model.clone() else {
            let error = Error::ServerError("No model selected");
            return Task::future(display_error(error)).discard();
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Task::done(ServerAction::Abort.into());
        };

        current_page.clear_content();

        let tasks = self.server_state.collect_task(current_page, &model, page);
        let task = self.method.join_tasks(tasks);
        let complete_task = self.bind_handle(Task::done(TransAction::PageComplete(page)));

        task.chain(complete_task)
            .chain(Task::done(ServerAction::Abort.into()))
    }

    pub fn translate_part(&mut self, page: usize, part: usize) -> Task<TransAction> {
        if !self.server_state.connected() {
            let error = Error::ServerError("Not connected to a server");
            return Task::future(display_error(error)).discard();
        }

        let Some(model) = self.server_state.current_model.clone() else {
            let error = Error::ServerError("No model selected");
            return Task::future(display_error(error)).discard();
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Task::done(ServerAction::Abort.into());
        };

        let Some(section) = current_page.sections.get_mut(part) else {
            log::error!("an invalid part was passed");
            return Task::none();
        };

        let text = current_page.text.get_mut(part).unwrap();
        text.clear();
        text.push_str(&part_tag(part + 1));

        let task = self
            .server_state
            .translate_part(section.clone(), model, page, part);

        task.chain(Task::done(ServerAction::Abort.into()))
    }

    pub fn bind_handle(&mut self, task: Task<TransAction>) -> Task<TransAction> {
        let (task, handle) = task.abortable();
        self.server_state.handles.push(handle.abort_on_drop());
        task
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

pub async fn get_pages(file_name: String, buffer: Vec<u8>) -> Result<(String, Vec<Page>)> {
    let mut epub = EpubDoc::from_reader(Cursor::new(buffer))?;

    let paths = get_ordered_path(&epub);

    let pages: Result<Vec<Page>> = paths
        .into_iter()
        .map(|path| {
            let html = epub.get_resource_str_by_path(&path).unwrap();
            (path, html)
        })
        .map(|(path, html)| {
            let markdown = convert_html(&html)?;
            let mut sections = Vec::new();
            if !markdown.is_empty() {
                let partitioned = partition_text(&markdown);
                sections = partitioned.chunks(3).map(|x| x.join(" ")).collect();
            }
            Ok(Page::new(path, sections))
        })
        .collect();

    Ok((file_name, pages?))
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
