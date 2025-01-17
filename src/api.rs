use poem_openapi::{payload::Json, OpenApi};

use crate::{
    backend::AppControllerBackend,
    types::{App, AppConfig, AppId, AppStatus},
};

#[derive(Clone)]
pub struct Api<B: AppControllerBackend + 'static>(B);

impl<B: AppControllerBackend> From<B> for Api<B> {
    fn from(backend: B) -> Self {
        Self(backend)
    }
}

#[OpenApi]
impl<B: AppControllerBackend> Api<B> {
    /// Create new app
    #[oai(path = "/app", method = "post")]
    async fn create_app(&self, config: Json<AppConfig>) -> Json<App> {
        todo!()
    }

    /// Start app
    ///
    /// The app must be created first.
    #[oai(path = "/app/{id}/start", method = "post")]
    async fn start_app(&self, id: Json<AppId>) -> Json<AppStatus> {
        todo!()
    }

    /// Stop app
    #[oai(path = "/app/{id}/stop", method = "post")]
    async fn stop_app(&self, id: Json<AppId>) -> Json<AppStatus> {
        todo!()
    }

    /// Delete app
    ///
    /// The app must be stopped first.
    #[oai(path = "/app/{id}", method = "delete")]
    async fn delete_app(&self, id: Json<AppId>) {
        todo!()
    }

    /// Get app
    #[oai(path = "/app/{id}", method = "get")]
    async fn get_app(&self, id: Json<AppId>) -> Json<App> {
        todo!()
    }

    /// Get all apps
    #[oai(path = "/app", method = "get")]
    async fn get_all_apps(&self) -> Json<Vec<App>> {
        todo!()
    }
}
