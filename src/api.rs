use poem_openapi::{param::Path, payload::Json, ApiResponse, OpenApi};

use crate::{
    backend::{AppControllerBackend, BackendError},
    types::{App, AppConfig, AppId, AppStatus, ContainerConfig, ImagePullPolicy, SocketAddr},
};

pub struct Api<B: AppControllerBackend + 'static>(B);

impl<B: AppControllerBackend> From<B> for Api<B> {
    fn from(backend: B) -> Self {
        Self(backend)
    }
}

#[derive(ApiResponse)]
enum CreateAppResponse {
    /// The app was created successfully.
    #[oai(status = 200)]
    Ok(Json<App>),
    /// The app config is too large. Maximum size is 1MB.
    #[oai(status = 413)]
    ConfigTooLarge,
    /// The app could not be created because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum StartAppResponse {
    /// The app was started successfully.
    #[oai(status = 200)]
    Ok(Json<AppStatus>),
    /// The app could not be started because it was not found.
    #[oai(status = 404)]
    NotFound,
    /// The app could not be started because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum StopAppResponse {
    /// The app was stopped successfully.
    #[oai(status = 200)]
    Ok(Json<AppStatus>),
    /// The app could not be stopped because it was not found.
    #[oai(status = 404)]
    NotFound,
    /// The app could not be stopped because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum DeleteAppResponse {
    /// The app was deleted successfully.
    #[oai(status = 204)]
    Ok,
    /// The app could not be deleted because it was not found.
    #[oai(status = 404)]
    NotFound,
    /// The app could not be deleted because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum GetAppResponse {
    /// The app was found successfully.
    #[oai(status = 200)]
    Ok(Json<App>),
    /// The app could not be found.
    #[oai(status = 404)]
    NotFound,
    /// The app could not be found because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum GetAllAppsResponse {
    /// The apps were found successfully.
    #[oai(status = 200)]
    Ok(Json<Vec<App>>),
    /// The apps could not be found because of an internal error.
    #[oai(status = 500)]
    InternalError(Json<String>),
}

#[derive(ApiResponse)]
enum GetAppAddrResponse {
    /// The address and port of the app were found successfully.
    #[oai(status = 200)]
    Ok(Json<SocketAddr>),
    /// The app could not be found.
    #[oai(status = 404)]
    NotFound,
    /// The app could not be found because of an internal error.
    #[oai(status = 500)]
    InternalError,
}

#[OpenApi]
impl<B: AppControllerBackend> Api<B> {
    /// Create new app
    ///
    /// Provide either 'images' (simple format) or 'containers' (advanced format), not both.
    /// If 'images' is provided, default container configurations will be created.
    /// If 'containers' is provided, those configurations will be used directly.
    /// Each container's config must be less than 1MB in size.
    #[oai(path = "/app", method = "post")]
    async fn create_app(&self, config: Json<AppConfig>) -> CreateAppResponse {
        // Check if any container config is too large (1MB = 1048576 bytes)
        for container_config in &config.0.containers {
            if let Some(config) = &container_config.config {
                if config.len() > 1048576 {
                    return CreateAppResponse::ConfigTooLarge;
                }
            }
        }

        // Validate that either images or containers is provided, not both
        if !config.0.images.is_empty() && !config.0.containers.is_empty() {
            return CreateAppResponse::InternalError(Json(
                "Provide either 'images' or 'containers', not both".to_string(),
            ));
        }

        // Validate that at least one container is specified
        if config.0.images.is_empty() && config.0.containers.is_empty() {
            return CreateAppResponse::InternalError(Json(
                "At least one container must be specified".to_string(),
            ));
        }

        // Normalize the config by converting images to containers if needed
        let mut normalized_config = config.0.clone();
        if !normalized_config.images.is_empty() {
            // Convert images to containers
            for image in &normalized_config.images {
                normalized_config.containers.push(ContainerConfig {
                    image: image.clone(),
                    config: None,
                    image_pull_policy: Some(ImagePullPolicy::IfNotPresent),
                });
            }
            // Clear the images field
            normalized_config.images.clear();
        }

        match self.0.create_app(normalized_config).await {
            Ok(app) => CreateAppResponse::Ok(Json(app)),
            Err(err) => CreateAppResponse::InternalError(Json(err.to_string())),
        }
    }

    /// Start app
    ///
    /// The app must be created first.
    #[oai(path = "/app/:id/start", method = "post")]
    async fn start_app(&self, id: Path<AppId>) -> StartAppResponse {
        match self.0.start_app(id.0).await {
            Ok(status) => StartAppResponse::Ok(Json(status)),
            Err(BackendError::NotFound) => StartAppResponse::NotFound,
            Err(BackendError::InternalError(msg)) => StartAppResponse::InternalError(Json(msg)),
        }
    }

    /// Stop app
    #[oai(path = "/app/:id/stop", method = "post")]
    async fn stop_app(&self, id: Path<AppId>) -> StopAppResponse {
        match self.0.stop_app(id.0).await {
            Ok(status) => StopAppResponse::Ok(Json(status)),
            Err(BackendError::NotFound) => StopAppResponse::NotFound,
            Err(BackendError::InternalError(msg)) => StopAppResponse::InternalError(Json(msg)),
        }
    }

    /// Delete app
    ///
    /// The app must be stopped first.
    #[oai(path = "/app/:id", method = "delete")]
    async fn delete_app(&self, id: Path<AppId>) -> DeleteAppResponse {
        match self.0.delete_app(id.0).await {
            Ok(()) => DeleteAppResponse::Ok,
            Err(BackendError::NotFound) => DeleteAppResponse::NotFound,
            Err(BackendError::InternalError(msg)) => DeleteAppResponse::InternalError(Json(msg)),
        }
    }

    /// Get app
    #[oai(path = "/app/:id", method = "get")]
    async fn get_app(&self, id: Path<AppId>) -> GetAppResponse {
        match self.0.get_app(id.0).await {
            Ok(app) => GetAppResponse::Ok(Json(app)),
            Err(BackendError::NotFound) => GetAppResponse::NotFound,
            Err(BackendError::InternalError(msg)) => GetAppResponse::InternalError(Json(msg)),
        }
    }

    /// Get all apps
    #[oai(path = "/app", method = "get")]
    async fn get_all_apps(&self) -> GetAllAppsResponse {
        match self.0.get_all_apps().await {
            Ok(apps) => GetAllAppsResponse::Ok(Json(apps)),
            Err(err) => GetAllAppsResponse::InternalError(Json(err.to_string())),
        }
    }

    /// Get internal app address
    #[oai(path = "/app/:id/addr", method = "get")]
    async fn get_app_addr(&self, id: Path<AppId>) -> GetAppAddrResponse {
        match self.0.get_app_addr(id.0).await {
            Ok((addr, port)) => GetAppAddrResponse::Ok(Json(SocketAddr {
                ip: addr.to_string(),
                port,
            })),
            Err(BackendError::NotFound) => GetAppAddrResponse::NotFound,
            Err(BackendError::InternalError(_)) => GetAppAddrResponse::InternalError,
        }
    }
}
