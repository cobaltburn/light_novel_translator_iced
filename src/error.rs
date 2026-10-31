use crate::message::display_error;
use iced::{Task, advanced::graphics::futures::MaybeSend};
use quick_xml::events::attributes::AttrError;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error("ServerError: {0}")]
    ServerError(&'static str),

    #[error("ConversionError path: {0:?}, {1:?}")]
    ConversionError(PathBuf, Box<Error>),

    #[error("BuildError: {0}")]
    BuildError(&'static str),

    #[error("Error: {0}")]
    Error(String),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    ArchiveError(#[from] rbook::ebook::errors::ArchiveError),

    #[error(transparent)]
    Ebookerror(#[from] rbook::ebook::errors::EbookError),

    #[error(transparent)]
    XmlError(#[from] quick_xml::Error),

    #[error(transparent)]
    EncodingError(#[from] quick_xml::encoding::EncodingError),

    #[error(transparent)]
    AttrError(#[from] AttrError),

    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error(transparent)]
    IconError(#[from] iced::window::icon::Error),

    #[error(transparent)]
    IcedError(#[from] iced::Error),

    #[error(transparent)]
    PromptError(#[from] rig::run::PromptError),

    #[error(transparent)]
    ProviderError(#[from] rig::ProviderError),

    #[error(transparent)]
    MemoryError(#[from] rig::memory::SessionError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Failed to save:\n{}", save_failures(.0))]
    SaveError(Vec<(PathBuf, std::io::Error)>),

    #[error("Page has no file name: {}", .0.display())]
    InvalidPagePath(PathBuf),

    #[error("Pages would overwrite each other:\n{}", page_collisions(.0))]
    PageNameCollision(Vec<Vec<PathBuf>>),
}

fn page_collisions(collisions: &[Vec<PathBuf>]) -> String {
    collisions
        .iter()
        .map(|paths| {
            paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn save_failures(failures: &[(PathBuf, std::io::Error)]) -> String {
    failures
        .iter()
        .map(|(path, error)| format!("{}: {error}", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Error {
    pub fn display_error<T: MaybeSend + 'static>(self) -> Task<T> {
        Task::future(display_error(self)).discard()
    }
}

pub trait TaskResultExt<T> {
    fn ok_or_display<M: Clone + MaybeSend + 'static>(
        self,
        f: impl Fn(T) -> Task<M> + MaybeSend + 'static,
    ) -> Task<M>;
}

impl<T, E> TaskResultExt<T> for Task<std::result::Result<T, E>>
where
    T: MaybeSend + 'static,
    E: Into<Error> + MaybeSend + 'static,
{
    fn ok_or_display<M: Clone + MaybeSend + 'static>(
        self,
        f: impl Fn(T) -> Task<M> + MaybeSend + 'static,
    ) -> Task<M> {
        self.then(move |result| match result {
            Ok(value) => f(value),
            Err(error) => Error::display_error(error.into()),
        })
    }
}

pub trait ResultTaskExt<T> {
    fn ok_or_display(self) -> Task<T>;
}

impl<T, E> ResultTaskExt<T> for std::result::Result<Task<T>, E>
where
    T: MaybeSend + 'static,
    E: Into<Error>,
{
    fn ok_or_display(self) -> Task<T> {
        self.unwrap_or_else(|error| Error::display_error(error.into()))
    }
}

impl<T, E> ResultTaskExt<T> for std::result::Result<(), E>
where
    T: MaybeSend + 'static,
    E: Into<Error> + MaybeSend + 'static,
{
    fn ok_or_display(self) -> Task<T> {
        match self {
            Ok(_) => Task::none(),
            Err(error) => Error::display_error(error.into()),
        }
    }
}
