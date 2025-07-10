use std::net::IpAddr;

use super::{AppControllerBackend, BackendError};
use crate::types::{App, AppConfig, AppId, AppStatus, ContainerIndex, ContainerOutput};

#[derive(Default)]
pub struct NullBackend;

#[async_trait::async_trait]
impl AppControllerBackend for NullBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        Ok(App { id: 0, config })
    }

    async fn start_app(&self, _id: AppId) -> Result<AppStatus, BackendError> {
        Ok(AppStatus::Ready)
    }

    async fn stop_app(&self, _id: AppId) -> Result<AppStatus, BackendError> {
        Ok(AppStatus::Stopped)
    }

    async fn delete_app(&self, _id: AppId) -> Result<(), BackendError> {
        Ok(())
    }

    async fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        Ok(App {
            id,
            config: AppConfig::default(),
        })
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        Ok(Vec::new())
    }

    async fn get_app_addr(&self, _id: AppId) -> Result<(IpAddr, u16), BackendError> {
        Ok((IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 8080))
    }

    async fn get_app_output(
        &self,
        id: AppId,
        index: ContainerIndex,
    ) -> Result<ContainerOutput, BackendError> {
        Ok(format!("Output for app {id} container {index}"))
    }

    async fn get_app_status(&self, _id: AppId) -> Result<AppStatus, BackendError> {
        Ok(AppStatus::NotReady)
    }
}
