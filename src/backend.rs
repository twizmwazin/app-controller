use crate::types::{App, AppConfig, AppId, AppStatus};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("The app could not be found.")]
    NotFound,
    #[error("An internal error occurred: {0}")]
    InternalError(String),
}

/// Backend trait for the app controller.
pub trait AppControllerBackend: Send + Sync {
    /// Create a new app with the given configuration.
    fn create_app(&self, config: AppConfig) -> Result<App, BackendError>;

    /// Start the app with the given ID.
    fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError>;

    /// Stop the app with the given ID.
    fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError>;

    /// Delete the app with the given ID.
    fn delete_app(&self, id: AppId) -> Result<(), BackendError>;

    /// Get the app with the given ID.
    fn get_app(&self, id: AppId) -> Result<App, BackendError>;

    /// Get all apps currently managed by the controller.
    fn get_all_apps(&self) -> Result<Vec<App>, BackendError>;
}

#[derive(Default)]
pub struct BackendImpl {}

impl AppControllerBackend for BackendImpl {
    fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        todo!()
    }

    fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        todo!()
    }

    fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        todo!()
    }

    fn delete_app(&self, id: AppId) -> Result<(), BackendError> {
        todo!()
    }

    fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        todo!()
    }

    fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        todo!()
    }
}
