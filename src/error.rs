use quick_xml::events::attributes::AttrError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    OllamaError(#[from] ollama_rs::error::OllamaError),

    #[error("ServerError: {0}")]
    ServerError(&'static str),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    DocError(#[from] epub::doc::DocError),

    #[error(transparent)]
    XmlError(#[from] quick_xml::Error),

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

pub type Result<T> = std::result::Result<T, Error>;
