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

    #[error(transparent)]
    OllamaError(#[from] ollama_rs::error::OllamaError),

    #[error("ServerError: {0}")]
    ServerError(&'static str),

    #[error("ConversionError path: {0:?}, {1:?}")]
    ConversionError(PathBuf, Box<Error>),

    #[error("BuildError: {0}")]
    BuildError(&'static str),

    #[error("GeneralError: {0}")]
    GeneralError(String),

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
}

impl Error {
    pub fn display_error<T: MaybeSend + 'static>(self) -> Task<T> {
        Task::future(display_error(self)).discard()
    }
}
