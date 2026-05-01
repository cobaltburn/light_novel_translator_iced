use crate::{
    actions::select_format_folder,
    controller::builder::DocBuilder,
    error::{Error, Result},
    message::{Message, select_epub},
    model::format::{Format, FormatPage},
};
use epub::doc::EpubDoc;
use iced::{Task, widget::image::Handle};
use std::{
    io::Cursor,
    mem,
    path::{Path, PathBuf},
};

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum FormatAction {
    SelectFolder,
    SetPages {
        name: String,
        pages: Vec<(PathBuf, String)>,
    },
    SelectEpub,
    SetEpub {
        path: PathBuf,
        buffer: Vec<u8>,
    },
    SetTitle(String),
    SetAuthors(String),
    Build,
}

impl Format {
    pub fn perform(&mut self, action: FormatAction) -> Task<Message> {
        match action {
            FormatAction::SetTitle(title) => self.set_title(title).into(),
            FormatAction::SetAuthors(authors) => self.set_authors(authors).into(),
            FormatAction::SetPages { name, pages } => self.set_pages(name, pages).into(),
            FormatAction::SelectEpub => Task::future(select_epub()).and_then(|(path, buffer)| {
                Task::done(FormatAction::SetEpub { path, buffer }.into())
            }),
            FormatAction::SelectFolder => Task::future(select_format_folder(
                self.epub_path.parent().unwrap_or(Path::new("")).into(),
            ))
            .and_then(|(name, pages)| Task::done(FormatAction::SetPages { name, pages }.into())),
            FormatAction::SetEpub { path, buffer } => match self.set_epub(path, buffer) {
                Ok(_) => Task::none(),
                Err(error) => error.display_error(),
            },
            FormatAction::Build => Task::done(self.get_build_content())
                .and_then(|builder| Task::done(builder.build()))
                .and_then(|(content, name)| Task::future(save_epub(content, name)))
                .then(|e| match e {
                    Ok(_) => Task::none(),
                    Err(error) => error.display_error(),
                }),
        }
    }

    fn set_pages(&mut self, name: String, pages: Vec<(PathBuf, String)>) {
        self.pages = pages.into_iter().map(|e| FormatPage::from(e)).collect();
        self.source_folder = name;
    }

    fn set_title(&mut self, title: String) {
        self.metadata.title = title
    }

    fn set_authors(&mut self, authors: String) {
        self.metadata.authors = authors
    }

    fn set_epub(&mut self, path: PathBuf, buffer: Vec<u8>) -> Result<()> {
        let mut epub = EpubDoc::from_reader(Cursor::new(buffer))?;
        let cover = epub.get_cover().map(|e| Handle::from_bytes(e.0));
        let authors = epub
            .metadata
            .iter()
            .find(|e| e.property == "creator")
            .map(|e| e.value.to_owned())
            .unwrap_or_default();

        let title = path
            .file_stem()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        self.metadata.title = title;
        self.metadata.authors = authors;
        self.epub_path = path;
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
