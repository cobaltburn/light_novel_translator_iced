use crate::{
    actions::select_format_folder,
    controller::DocBuilder,
    error::{Error, Result, ResultTaskExt as _, TaskResultExt},
    message::select_epub,
    model::{Format, FormatPage},
};
use iced::{Task, widget::image::Handle};
use rbook::Epub;
use std::{io::Cursor, mem, path::PathBuf};

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Action {
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
    #[allow(clippy::unit_arg)]
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::SetTitle(title) => self.set_title(title).into(),
            Action::SetAuthors(authors) => self.set_authors(authors).into(),
            Action::SetPages { name, pages } => self.set_pages(name, pages).into(),
            Action::SelectEpub => Task::future(select_epub())
                .and_then(|(path, buffer)| Task::done(Action::SetEpub { path, buffer })),
            Action::SelectFolder => Task::future(select_format_folder(
                self.epub_path.parent().map_or(PathBuf::new(), Into::into),
            ))
            .and_then(|(name, pages)| Task::done(Action::SetPages { name, pages })),
            Action::SetEpub { path, buffer } => self.set_epub(path, buffer).ok_or_display(),
            Action::Build => Task::done(self.get_build_content())
                .and_then(|builder| Task::done(builder.build()))
                .and_then(|(content, name)| Task::future(save_epub(content, name)))
                .ok_or_display(Into::into),
        }
    }

    fn set_pages(&mut self, name: String, pages: Vec<(PathBuf, String)>) {
        self.pages = pages.into_iter().map(FormatPage::from).collect();
        self.source_folder = name;
    }

    fn set_title(&mut self, title: String) {
        self.metadata.title = title
    }

    fn set_authors(&mut self, authors: String) {
        self.metadata.authors = authors
    }

    fn set_epub(&mut self, path: PathBuf, buffer: Vec<u8>) -> Result<()> {
        let epub = Epub::read(Cursor::new(buffer))?;

        self.cover = epub
            .manifest()
            .cover_image()
            .and_then(|e| Some(Handle::from_bytes(e.read_bytes().ok()?)));
        self.metadata.title = path
            .file_stem()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        self.metadata.authors = epub
            .metadata()
            .by_property("creator")
            .map(|e| e.value().to_owned())
            .collect();
        self.epub_path = path;
        self.epub = Some(epub);

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

        Ok(DocBuilder::new(epub, name, pages, metadata))
    }
}

pub async fn save_epub(content: Vec<u8>, file_name: impl Into<String>) -> Result<()> {
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
