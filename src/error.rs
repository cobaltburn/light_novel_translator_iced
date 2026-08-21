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
    DocError(#[from] epub::doc::DocError),

    #[error(transparent)]
    XmlError(#[from] quick_xml::Error),

    #[error(transparent)]
    EncodingError(#[from] quick_xml::encoding::EncodingError),

    #[error(transparent)]
    AttrError(#[from] AttrError),

    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),

    #[error(transparent)]
    EpubBuilderError(#[from] epub_builder::Error),

    #[error(transparent)]
    IconError(#[from] iced::window::icon::Error),

    #[error(transparent)]
    IcedError(#[from] iced::Error),

    #[error(transparent)]
    ModelError(#[from] rig_core::model::ModelListingError),

    #[error(transparent)]
    StreamError(#[from] rig_core::agent::StreamingError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
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

impl<T: MaybeSend + 'static> TaskResultExt<T> for Task<Result<T>> {
    fn ok_or_display<M: Clone + MaybeSend + 'static>(
        self,
        f: impl Fn(T) -> Task<M> + MaybeSend + 'static,
    ) -> Task<M> {
        self.then(move |result| match result {
            Ok(value) => f(value),
            Err(error) => error.display_error(),
        })
    }
}

pub trait ResultTaskExt<T> {
    fn ok_or_display(self) -> Task<T>;
}

impl<T: MaybeSend + 'static> ResultTaskExt<T> for Result<Task<T>> {
    fn ok_or_display(self) -> Task<T> {
        self.unwrap_or_else(Error::display_error)
    }
}

impl<T: MaybeSend + 'static> ResultTaskExt<T> for Result<()> {
    fn ok_or_display(self) -> Task<T> {
        match self {
            Ok(_) => Task::none(),
            Err(error) => error.display_error(),
        }
    }
}
