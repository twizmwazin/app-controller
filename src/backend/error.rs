use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("The app could not be found.")]
    NotFound,
    #[error("An internal error occurred: {0}")]
    InternalError(String),
}

impl From<serde_json::Error> for BackendError {
    fn from(err: serde_json::Error) -> Self {
        Self::InternalError(err.to_string())
    }
}

impl From<kube::Error> for BackendError {
    fn from(err: kube::Error) -> Self {
        match err {
            kube::Error::Api(err) if err.code == 404 => Self::NotFound,
            _ => Self::InternalError(err.to_string()),
        }
    }
}
