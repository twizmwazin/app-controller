use std::future::Future;

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
    fn create_app(
        &self,
        config: AppConfig,
    ) -> impl Future<Output = Result<App, BackendError>> + Send;

    /// Start the app with the given ID.
    fn start_app(&self, id: AppId) -> impl Future<Output = Result<AppStatus, BackendError>> + Send;

    /// Stop the app with the given ID.
    fn stop_app(&self, id: AppId) -> impl Future<Output = Result<AppStatus, BackendError>> + Send;

    /// Delete the app with the given ID.
    fn delete_app(&self, id: AppId) -> impl Future<Output = Result<(), BackendError>> + Send;

    /// Get the app with the given ID.
    fn get_app(&self, id: AppId) -> impl Future<Output = Result<App, BackendError>> + Send;

    /// Get all apps currently managed by the controller.
    fn get_all_apps(&self) -> impl Future<Output = Result<Vec<App>, BackendError>> + Send;
}

#[derive(Default)]
pub struct BackendImpl {}

impl AppControllerBackend for BackendImpl {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        todo!()
    }

    async fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        todo!()
    }

    async fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        todo!()
    }

    async fn delete_app(&self, id: AppId) -> Result<(), BackendError> {
        todo!()
    }

    async fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        todo!()
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        todo!()
    }
}
