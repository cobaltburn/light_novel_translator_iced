use crate::{
    controller::builder::epub::{BuilderPage, DocBuilder},
    error::Result,
    message::{Message, display_error, open_epub},
    state::format_model::{FormatModel, FormatPage},
};
use epub::doc::EpubDoc;
use epub_builder::{EpubBuilder, ZipLibrary};
use iced::{Task, widget::text_editor};
use std::{ffi::OsStr, fs::read_dir, io::Cursor, mem, path::PathBuf};
use tokio::fs::read_to_string;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum FormatAction {
    SetPage(usize),
    SelectFolder,
    SetPages(String, Vec<(PathBuf, String)>),
    EditContent(text_editor::Action),
    SelectEpub,
    SetEpub((String, Vec<u8>)),
    Build,
}

impl FormatModel {
    pub fn perform(&mut self, action: FormatAction) -> Task<Message> {
        match action {
            FormatAction::SelectEpub => Task::future(open_epub())
                .and_then(|doc| Task::done(FormatAction::SetEpub(doc).into())),
            FormatAction::SelectFolder => Task::future(select_format_folder())
                .and_then(|(f, p)| Task::done(FormatAction::SetPages(f, p).into())),
            FormatAction::SetPage(page) => self.set_current_page(page).into(),
            FormatAction::SetPages(folder, pages) => self.set_pages(folder, pages).into(),
            FormatAction::EditContent(action) => self.edit_current_content(action).into(),
            FormatAction::SetEpub(doc) => Task::done(self.set_epub(doc)).then(|r| match r {
                Ok(_) => Task::none(),
                Err(error) => Task::future(display_error(error)).discard(),
            }),
            FormatAction::Build => Task::done(self.get_build_content())
                .and_then(|builder| Task::done(builder.build()))
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

    fn set_current_page(&mut self, page: usize) {
        self.current_page = Some(page);
    }

    fn set_pages(&mut self, folder: String, pages: Vec<(PathBuf, String)>) {
        self.pages = pages.into_iter().map(|e| FormatPage::from(e)).collect();
        self.current_page = Some(0);
        self.source_folder = folder;
    }

    fn edit_current_content(&mut self, action: text_editor::Action) {
        let Some(i) = self.current_page else {
            return;
        };

        if let Some(page) = self.pages.get_mut(i) {
            page.content.perform(action);
        };
    }

    fn set_epub(&mut self, (name, buffer): (String, Vec<u8>)) -> Result<()> {
        let epub = EpubDoc::from_reader(Cursor::new(buffer))?;

        self.epub_name = name;
        self.epub = Some(epub);
        Ok(())
    }

    pub fn get_build_content(&mut self) -> Option<DocBuilder> {
        let name = mem::take(&mut self.epub_name);
        let epub = mem::take(&mut self.epub)?;
        let pages = mem::take(&mut self.pages);
        let pages = pages.into_iter().map(BuilderPage::from).collect();
        let builder = EpubBuilder::new(ZipLibrary::new().ok()?).ok()?;
        self.source_folder.clear();

        let doc_builder = DocBuilder {
            epub,
            name,
            pages,
            builder,
        };
        Some(doc_builder)
    }
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
