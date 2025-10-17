use crate::{
    controller::builder::epub::{BuilderPage, DocBuilder},
    message::{Message, open_epub},
};
use epub::{archive::EpubArchive, doc::EpubDoc};
use iced::{
    Task,
    widget::text_editor::{self, Content},
};
use std::{ffi::OsStr, fs::read_dir, io::Cursor, path::PathBuf};
use tokio::fs::{self, read_to_string};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatModel {
    pub pages: Vec<FormatPage>,
    pub current_page: Option<usize>,
    pub toc_path: Option<PathBuf>,
    pub source_folder: String,
    pub epub_name: String,
    pub builder: Option<DocBuilder>,
}

impl FormatModel {
    pub fn current_content(&self) -> Option<&Content> {
        let page = self.current_page?;
        self.pages.get(page).map(|e| &e.content)
    }

    fn set_current_page(&mut self, page: usize) -> Task<Message> {
        self.current_page = Some(page);
        Task::none()
    }

    fn set_pages(&mut self, pages: Vec<(PathBuf, String)>) -> Task<Message> {
        self.pages = pages.into_iter().map(|e| FormatPage::from(e)).collect();
        self.current_page = Some(0);
        Task::none()
    }

    fn edit_current_content(&mut self, action: text_editor::Action) -> Task<Message> {
        let Some(i) = self.current_page else {
            return Task::none();
        };
        if let Some(page) = self.pages.get_mut(i) {
            page.content.perform(action);
            page.changed = true;
        };

        Task::none()
    }

    fn set_toc(&mut self, toc: PathBuf) -> Task<Message> {
        self.toc_path = Some(toc);
        Task::none()
    }

    fn set_folder(&mut self, folder: String) -> Task<Message> {
        self.source_folder = folder;
        Task::none()
    }

    fn set_epub(&mut self, (name, buffer): (String, Vec<u8>)) -> Task<Message> {
        let epub = match EpubDoc::from_reader(Cursor::new(buffer.clone())) {
            Ok(epub) => epub,
            Err(error) => return Task::done(Message::Error(format!("{:#?}", error))),
        };

        let archive = match EpubArchive::from_reader(Cursor::new(buffer)) {
            Ok(archive) => archive,
            Err(error) => return Task::done(Message::Error(format!("{:#?}", error))),
        };

        self.epub_name = name;
        self.builder = Some(DocBuilder { epub, archive });
        Task::none()
    }

    fn mark_saved(&mut self, page: usize) -> Task<Message> {
        if let Some(page) = self.pages.get_mut(page) {
            page.changed = false;
        }
        Task::none()
    }

    pub fn save_files(&self) -> Task<Message> {
        let tasks = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.changed)
            .map(|(i, page)| {
                Task::future(fs::write(page.path.clone(), page.content.text()))
                    .and_then(move |_| Task::done(FormatAction::MarkSaved(i).into()))
            });
        Task::batch(tasks)
    }

    pub fn get_build_content(
        &self,
    ) -> Option<(DocBuilder, Option<PathBuf>, String, Vec<BuilderPage>)> {
        let doc_builder = self.builder.clone()?;
        let toc = self.toc_path.clone();
        let epub_name = self.epub_name.clone();
        let pages = self
            .pages
            .iter()
            .map(|FormatPage { path, content, .. }| BuilderPage {
                path: path.clone(),
                content: content.text(),
            })
            .collect::<Vec<BuilderPage>>();
        Some((doc_builder, toc, epub_name, pages))
    }

    pub fn perform(&mut self, action: FormatAction) -> Task<Message> {
        match action {
            FormatAction::SelectEpub => Task::future(open_epub())
                .and_then(|doc| Task::done(FormatAction::SetEpub(doc).into())),
            FormatAction::SelectFolder => {
                Task::future(select_format_folder()).and_then(|(folder, pages)| {
                    Task::done(FormatAction::SetPages(pages).into())
                        .chain(Task::done(FormatAction::SetFolder(folder).into()))
                })
            }
            FormatAction::SelectToc => Task::future(select_toc_file())
                .and_then(|path| Task::done(FormatAction::SetToc(path).into())),
            FormatAction::SetPage(page) => self.set_current_page(page),
            FormatAction::SetPages(pages) => self.set_pages(pages),
            FormatAction::EditContent(action) => self.edit_current_content(action),
            FormatAction::SaveFiles => self.save_files(),
            FormatAction::SetToc(path) => self.set_toc(path),
            FormatAction::MarkSaved(page) => self.mark_saved(page),
            FormatAction::SetEpub(doc) => self.set_epub(doc),
            FormatAction::SetFolder(folder) => self.set_folder(folder),
            FormatAction::Build => match self.get_build_content() {
                Some((builder, toc, name, pages)) => {
                    Task::perform(builder.build(toc, name, pages), |e| match e {
                        Ok(_) => Message::None,
                        Err(error) => Message::Error(format!("{:#?}", error)),
                    })
                    .discard()
                }
                None => Task::none(),
            },
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum FormatAction {
    SetPage(usize),
    SelectFolder,
    SetFolder(String),
    SetPages(Vec<(PathBuf, String)>),
    SetToc(PathBuf),
    MarkSaved(usize),
    EditContent(text_editor::Action),
    SaveFiles,
    SelectToc,
    SelectEpub,
    SetEpub((String, Vec<u8>)),
    Build,
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatPage {
    pub path: PathBuf,
    pub content: text_editor::Content,
    pub changed: bool,
}

impl<S: AsRef<str>> From<(PathBuf, S)> for FormatPage {
    fn from((path, content): (PathBuf, S)) -> Self {
        FormatPage {
            path,
            content: Content::with_text(content.as_ref()),
            changed: false,
        }
    }
}

async fn select_toc_file() -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select table of contents")
        .pick_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn select_format_folder() -> Option<(String, Vec<(PathBuf, String)>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select translated folder")
        .pick_folder()
        .await?;

    let read_dir = read_dir(handle.path()).ok()?;
    let paths = read_dir.flatten().flat_map(|entry| {
        let path = entry.path();
        let path = path.is_file().then_some(path)?;
        if path.extension() == Some(&OsStr::new("md")) {
            Some(path)
        } else {
            None
        }
    });
    let mut pages = Vec::new();
    for path in paths {
        let content = read_to_string(&path).await.ok()?;
        pages.push((path, content));
    }
    Some((handle.file_name(), pages))
}
