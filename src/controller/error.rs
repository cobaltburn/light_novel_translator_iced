use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    OllamaError(#[from] ollama_rs::error::OllamaError),

    #[error("ServerError: {0}")]
    ServerError(String),

    #[error("IOError: {0}")]
    IOError(std::io::ErrorKind),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IOError(error.kind())
    }
}
