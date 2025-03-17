use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("The app could not be found.")]
    NotFound,
    #[error("Container index is invalid.")]
    InvalidContainerIndex,
    #[error("An internal error occurred: {0}")]
    InternalError(String),
}

impl From<kube::Error> for BackendError {
    fn from(err: kube::Error) -> Self {
        match err {
            kube::Error::Api(err) if err.code == 404 => Self::NotFound,
            _ => Self::InternalError(err.to_string()),
        }
    }
}

impl From<std::string::FromUtf8Error> for BackendError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::InternalError(err.to_string())
    }
}

impl<T: poem_openapi::types::Type> From<poem_openapi::types::ParseError<T>> for BackendError {
    fn from(err: poem_openapi::types::ParseError<T>) -> Self {
        Self::InternalError(err.into_message())
    }
}
