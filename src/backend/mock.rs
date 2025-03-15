use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        RwLock,
    },
};

use super::{AppControllerBackend, BackendError};
use crate::types::{App, AppConfig, AppId, AppStatus, ContainerOutput};

#[derive(Default)]
pub struct MockBackend {
    next_id: AtomicU64,
    apps: RwLock<HashMap<AppId, App>>,
    statuses: RwLock<HashMap<AppId, AppStatus>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AppControllerBackend for MockBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let app = App { id, config };

        self.apps.write().unwrap().insert(id, app.clone());
        self.statuses
            .write()
            .unwrap()
            .insert(id, AppStatus::Stopped);

        Ok(app)
    }

    async fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        if !self.apps.read().unwrap().contains_key(&id) {
            return Err(BackendError::NotFound);
        }

        self.statuses
            .write()
            .unwrap()
            .insert(id, AppStatus::Running);
        Ok(AppStatus::Running)
    }

    async fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        if !self.apps.read().unwrap().contains_key(&id) {
            return Err(BackendError::NotFound);
        }

        self.statuses
            .write()
            .unwrap()
            .insert(id, AppStatus::Stopped);
        Ok(AppStatus::Stopped)
    }

    async fn delete_app(&self, id: AppId) -> Result<(), BackendError> {
        if self.apps.write().unwrap().remove(&id).is_none() {
            return Err(BackendError::NotFound);
        }
        self.statuses.write().unwrap().remove(&id);
        Ok(())
    }

    async fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        self.apps
            .read()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(BackendError::NotFound)
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        Ok(self.apps.read().unwrap().values().cloned().collect())
    }

    async fn get_app_addr(&self, _id: AppId) -> Result<(IpAddr, u16), BackendError> {
        todo!()
    }

    async fn get_app_outputs(&self, id: AppId) -> Result<Vec<ContainerOutput>, BackendError> {
        // Check if the app exists
        let app = self.get_app(id).await?;

        // For the mock backend, we'll just return placeholder outputs
        let mut outputs = Vec::new();

        // Create a mock output for each container in the app
        for (index, container) in app.config.get_containers().iter().enumerate() {
            outputs.push(ContainerOutput {
                container_index: index,
                container_name: format!("container-{}", index),
                output: format!(
                    "Mock output for container {} ({})",
                    index,
                    container.image()
                ),
            });
        }

        Ok(outputs)
    }
}
