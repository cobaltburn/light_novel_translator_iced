use crate::{
    controller::{
        doc::get_ordered_path,
        markdown::{convert_html, partition_text},
        xml::remove_think_tags,
    },
    error::Error,
    message::{Message, display_error, open_epub},
    state::server_state::{ServerAction, ServerState},
};
use epub::doc::EpubDoc;
use iced::Task;
use std::{
    collections::HashMap,
    io::Cursor,
    mem::swap,
    path::{Path, PathBuf},
};
use tokio::fs;

#[derive(Default, Debug)]
pub struct TranslationModel {
    pub server_state: ServerState,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub file_name: String,
    pub current_page: Option<usize>,
    pub pages: Vec<Page>,
    pub file_drop_down: bool,
}

impl TranslationModel {
    pub fn update_content(&mut self, text: String, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.content.push_str(&text);
        }
    }

    pub fn current_content(&self) -> Option<&str> {
        Some(&self.pages.get(self.current_page?)?.content)
    }

    pub fn set_current_page(&mut self, page: usize) {
        self.current_page = Some(page);
    }

    pub fn mark_complete(&mut self, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.complete = true;
        }
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<Message> {
        let tasks = self
            .pages
            .iter()
            .filter(|page| page.complete)
            .filter(|page| !page.content.is_empty())
            .map(|page| {
                let name = page.path.file_stem().unwrap().to_os_string();
                (name, page.content.clone())
            })
            .map(|(name, text)| {
                let text = remove_think_tags(&text);
                (name, text)
            })
            .map(|(name, text)| {
                let path = path.clone();
                let file_path = path.join(name).with_extension("md");
                Task::future(async move { fs::write(file_path, text.as_bytes()).await }).then(|r| {
                    match r {
                        Ok(_) => Task::none(),
                        Err(error) => Task::future(display_error(error)),
                    }
                })
            });
        Task::batch(tasks).discard()
    }

    fn load_pages(&mut self, pages: Vec<(PathBuf, String)>) {
        let mut pages: HashMap<String, String> = pages
            .into_iter()
            .filter_map(|(p, c)| {
                let p = p.file_stem()?.to_string_lossy().into_owned();
                Some((p, c))
            })
            .collect();

        for page in self.pages.iter_mut() {
            let file_name = page.path.file_stem().unwrap_or_default();
            let file_name = file_name.to_string_lossy().into_owned();
            if let Some(content) = pages.get_mut(&file_name) {
                swap(&mut page.content, content);
                page.complete = true;
            }
        }
        let last = self.pages.iter().filter(|p| p.complete).last();
        if let Some(path) = last.map(|p| p.path.clone()) {
            let pages = self.pages.iter_mut().take_while(|p| p.path != path);
            pages.for_each(|p| p.complete = true);
        }
    }

    pub fn set_epub(&mut self, file_name: String, buffer: Vec<u8>) -> Task<Message> {
        let epub = match EpubDoc::from_reader(Cursor::new(buffer)) {
            Ok(epub) => epub,
            Err(error) => return Task::future(display_error(error)).discard(),
        };

        let paths = get_ordered_path(&epub);
        self.pages = paths.into_iter().map(Into::into).collect();
        self.current_page = Some(0);
        self.file_name = file_name;
        self.epub = Some(epub);

        Task::none()
    }

    pub fn execute_translation(&mut self, page: usize) -> Task<Message> {
        if !self.server_state.connected() {
            return Task::future(display_error(Error::ServerError(
                "Not connected to a server",
            )))
            .discard();
        }
        let Some(model) = self.server_state.current_model.clone() else {
            return Task::future(display_error(Error::ServerError("No model selected"))).discard();
        };

        let Some(epub) = self.epub.as_mut() else {
            return Task::future(display_error(Error::ServerError("No epub selected"))).discard();
        };

        let Some(current_page) = self.pages.get_mut(page) else {
            return Task::done(ServerAction::Abort.into());
        };

        current_page.content.clear();
        epub.set_current_chapter(page);
        let html = epub.get_current_str().expect("max page exceeded").0;

        let markdown = match convert_html(&html) {
            Ok(markdown) => markdown,
            Err(error) => {
                return Task::future(display_error(error)).discard();
            }
        };

        if markdown.is_empty() {
            let mark_task = Task::done(TransAction::PageComplete(page).into());
            let next_task = Task::done(TransAction::Translate(page + 1).into());
            return mark_task.chain(next_task);
        }

        let partitioned = partition_text(&markdown);
        let sections = partitioned.chunks(3).map(|x| x.to_vec());

        let mut tasks = Vec::new();
        for (i, sections) in sections.enumerate() {
            let tag = format!("\n\n<part>{}</part>\n\n", i + 1);
            tasks.push(Task::done(TransAction::UpdateContent(tag, page).into()));

            let server = self.server_state.server.clone();
            let settings = self.server_state.settings.clone();
            tasks.push(server.translate(model.clone(), sections, page, settings));
        }

        tasks.push(Task::done(TransAction::PageComplete(page).into()));
        tasks.push(Task::done(TransAction::Translate(page + 1).into()));

        let mut task_chain = Task::none();
        for (task, handle) in tasks.into_iter().map(Task::abortable) {
            task_chain = task_chain.chain(task);
            self.server_state.handles.push(handle);
        }

        task_chain
    }

    pub fn perform(&mut self, action: TransAction) -> Task<Message> {
        match action {
            TransAction::ServerAction(action) => self.server_state.perform(action),
            TransAction::SetPage(page) => self.set_current_page(page).into(),
            TransAction::UpdateContent(text, page) => self.update_content(text, page).into(),
            TransAction::PageComplete(page) => self.mark_complete(page).into(),
            TransAction::Translate(page) => self.execute_translation(page),
            TransAction::SavePages(path) => self.save_pages(path),
            TransAction::LoadPages(pages) => self.load_pages(pages).into(),
            TransAction::SetEpub((file_name, buffer)) => self.set_epub(file_name, buffer),
            TransAction::OpenEpub => Task::future(open_epub())
                .and_then(|doc| Task::done(TransAction::SetEpub(doc).into())),
            TransAction::LoadTranslation => Task::future(load_folder_markdown())
                .and_then(|pages| Task::done(TransAction::LoadPages(pages).into())),
            TransAction::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .then(|path| match path {
                    Ok(path) => Task::done(TransAction::SavePages(path).into()),
                    Err(err) => Task::future(display_error(err)).discard(),
                }),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TransAction {
    SetPage(usize),
    UpdateContent(String, usize),
    PageComplete(usize),
    SavePages(PathBuf),
    LoadPages(Vec<(PathBuf, String)>),
    OpenEpub,
    SetEpub((String, Vec<u8>)),
    Translate(usize),
    SaveTranslation(String),
    LoadTranslation,
    ServerAction(ServerAction),
}

impl From<ServerAction> for TransAction {
    fn from(action: ServerAction) -> Self {
        TransAction::ServerAction(action)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub content: String,
    pub complete: bool,
}

impl Page {
    pub fn new(path: PathBuf) -> Self {
        Page {
            path,
            content: String::new(),
            complete: false,
        }
    }
}

impl From<PathBuf> for Page {
    fn from(path: PathBuf) -> Self {
        Page {
            path,
            content: String::new(),
            complete: false,
        }
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
