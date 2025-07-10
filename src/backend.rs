mod error;
mod kubernetes;
#[cfg(test)]
mod mock;

pub use error::BackendError;
pub use kubernetes::KubernetesBackend;
#[cfg(test)]
pub use mock::MockBackend;

use std::net::IpAddr;

use crate::types::{App, AppConfig, AppId, AppStatus, ContainerIndex, ContainerOutput};

/// Backend trait for the app controller.
#[async_trait::async_trait]
pub trait AppControllerBackend: Send + Sync {
    /// Create a new app with the given configuration.
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError>;

    /// Start the app with the given ID.
    async fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError>;

    /// Stop the app with the given ID.
    async fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError>;

    /// Delete the app with the given ID.
    async fn delete_app(&self, id: AppId) -> Result<(), BackendError>;

    /// Get the status of the app with the given ID.
    async fn get_app_status(&self, id: AppId) -> Result<AppStatus, BackendError>;

    /// Get the app with the given ID.
    async fn get_app(&self, id: AppId) -> Result<App, BackendError>;

    /// Get all apps currently managed by the controller.
    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError>;

    /// Get the address and port of the app with the given ID.
    async fn get_app_addr(&self, id: AppId) -> Result<(IpAddr, u16), BackendError>;

    /// Get the outputs from all containers in the app with the given ID.
    async fn get_app_output(
        &self,
        id: AppId,
        container_index: ContainerIndex,
    ) -> Result<ContainerOutput, BackendError>;
}
