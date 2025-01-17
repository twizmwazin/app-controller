use crate::types::{App, AppConfig, AppId, AppStatus};

/// Backend trait for the app controller.
pub trait AppControllerBackend: Send + Sync {
    /// Create a new app with the given configuration.
    fn create_app(&self, config: AppConfig) -> App;

    /// Start the app with the given ID.
    fn start_app(&self, id: AppId) -> AppStatus;

    /// Stop the app with the given ID.
    fn stop_app(&self, id: AppId) -> AppStatus;

    /// Delete the app with the given ID.
    fn delete_app(&self, id: AppId);

    /// Get the app with the given ID.
    fn get_app(&self, id: AppId) -> App;

    /// Get all apps currently managed by the controller.
    fn get_all_apps(&self) -> Vec<App>;
}

#[derive(Default)]
pub struct BackendImpl {}

impl AppControllerBackend for BackendImpl {
    fn create_app(&self, config: AppConfig) -> App {
        todo!()
    }

    fn start_app(&self, id: AppId) -> AppStatus {
        todo!()
    }

    fn stop_app(&self, id: AppId) -> AppStatus {
        todo!()
    }

    fn delete_app(&self, id: AppId) {
        todo!()
    }

    fn get_app(&self, id: AppId) -> App {
        todo!()
    }

    fn get_all_apps(&self) -> Vec<App> {
        todo!()
    }
}
