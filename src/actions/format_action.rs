use crate::{
    controller::builder::DocBuilder,
    error::{Error, Result},
    message::{Message, display_error, select_epub},
    model::format::{Format, FormatPage},
};
use epub::doc::EpubDoc;
use iced::{Task, widget::image::Handle};
use std::{
    ffi::OsStr,
    fs::read_dir,
    io::Cursor,
    mem,
    path::{Path, PathBuf},
};
use tokio::fs::read_to_string;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum FormatAction {
    SelectFolder,
    SetPages(String, Vec<(PathBuf, String)>),
    SelectEpub,
    SetEpub((PathBuf, Vec<u8>)),
    SetTitle(String),
    SetAuthors(String),
    Build,
}

impl Format {
    pub fn perform(&mut self, action: FormatAction) -> Task<Message> {
        match action {
            FormatAction::SetTitle(title) => (self.metadata.title = title).into(),
            FormatAction::SetAuthors(authors) => (self.metadata.authors = authors).into(),
            FormatAction::SetPages(folder, pages) => self.set_pages(folder, pages).into(),
            FormatAction::SelectEpub => Task::future(select_epub())
                .and_then(|doc| Task::done(FormatAction::SetEpub(doc).into())),
            FormatAction::SelectFolder => {
                Task::future(select_format_folder(self.epub_path.clone()))
                    .and_then(|(f, p)| Task::done(FormatAction::SetPages(f, p).into()))
            }
            FormatAction::SetEpub(doc) => Task::done(self.set_epub(doc)).then(|r| match r {
                Ok(_) => Task::none(),
                Err(error) => Task::future(display_error(error)).discard(),
            }),
            FormatAction::Build => Task::done(self.get_build_content())
                .then(|builder| match builder {
                    Ok(builder) => Task::done(builder.build()),
                    Err(error) => Task::done(Err(error)),
                })
                .then(|content| match content {
                    Ok((content, name)) => Task::future(save_epub(content, name)),
                    Err(error) => Task::done(Err(error)),
                })
                .then(|e| match e {
                    Ok(_) => Task::none(),
                    Err(error) => Task::future(display_error(error)),
                })
                .discard(),
        }
    }

    fn set_pages(&mut self, folder: String, pages: Vec<(PathBuf, String)>) {
        self.pages = pages.into_iter().map(|e| FormatPage::from(e)).collect();
        self.source_folder = folder;
    }

    fn set_epub(&mut self, (name, buffer): (PathBuf, Vec<u8>)) -> Result<()> {
        let mut epub = EpubDoc::from_reader(Cursor::new(buffer))?;
        let cover = epub.get_cover().map(|e| Handle::from_bytes(e.0));
        let authors = epub
            .metadata
            .iter()
            .find(|e| e.property == "creator")
            .map(|e| e.value.to_owned())
            .unwrap_or_default();

        let title = name
            .file_stem()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        self.metadata.title = title;
        self.metadata.authors = authors;
        self.epub_path = name;
        self.epub = Some(epub);
        self.cover = cover;

        Ok(())
    }

    pub fn get_build_content(&mut self) -> Result<DocBuilder> {
        let epub = mem::take(&mut self.epub).ok_or(Error::BuildError("Epub not found"))?;
        let pages = mem::take(&mut self.pages);
        let name = mem::take(&mut self.epub_path)
            .file_name()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let metadata = mem::take(&mut self.metadata);

        self.source_folder.clear();
        self.cover = None;

        DocBuilder::new(epub, name, pages, metadata)
    }
}

pub async fn select_format_folder(path: PathBuf) -> Option<(String, Vec<(PathBuf, String)>)> {
    let path = path.parent().unwrap_or(&Path::new(""));
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select translated folder")
        .set_directory(path)
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

pub async fn save_epub<T: Into<String>>(content: Vec<u8>, file_name: T) -> Result<()> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save epub")
        .set_file_name(file_name)
        .add_filter("epub", &["epub"])
        .save_file()
        .await;

    if let Some(handle) = handle {
        handle.write(&content).await?;
    }
    Ok(())
}
