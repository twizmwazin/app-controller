use std::str::FromStr;

use crate::types::{App, AppConfig, AppId, AppStatus, InteractionModel};
use k8s_openapi::api::{apps::v1::Deployment, core::v1::Service};
use kube::{
    api::{ListParams, Patch},
    Api, Client,
};
use poem_openapi::types::ParseFromParameter;

use super::{AppControllerBackend, BackendError};

pub struct KubernetesBackend {
    client: Client,
}

impl KubernetesBackend {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    async fn get_deployment(&self, id: AppId) -> Result<Deployment, BackendError> {
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());
        // Find the deployment with the correct app-controller-id label
        let list_params = ListParams::default().labels(&format!("app-controller-id={}", id));
        let list = deployment_api.list(&list_params).await?;

        // There should be only one deployment with the given ID.
        let deployment = list
            .items
            .into_iter()
            .next()
            .ok_or(BackendError::NotFound)?;

        Ok(deployment)
    }
}

impl AppControllerBackend for KubernetesBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        // To creat an app, we need a service and a deployment. The deployment
        // should have zero replicas initially.

        let unique_id: u32 = rand::random();
        let name = format!("{}-{:08x}", config.name, unique_id);

        let labels = serde_json::json!({
            "app-controller-id": format!("{}", unique_id),
            "app-controller-name": config.name,
            "app-controller-interaction-model": config.interaction_model,
            "app-controller-image": config.image,
        });

        let service: Service = serde_json::from_value(serde_json::json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": name,
                "labels": labels,
            },
            "spec": {
                "selector": {
                    "app": name,
                },
                "ports": [{
                    "protocol": "TCP",
                    "port": 80,
                    "targetPort": 5910,
                }],
            },
        }))?;

        let deployment: Deployment = serde_json::from_value(serde_json::json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": name,
                "labels": labels,
            },
            "spec": {
                "replicas": 0,
                "selector": {
                    "matchLabels": {
                        "app": name,
                    },
                },
                "template": {
                    "metadata": {
                        "labels": {
                            "app": name,
                        },
                    },
                    "spec": {
                        "containers": [{
                        "name": name,
                        "image": config.image,
                        "ports": [{
                            "containerPort": 5910,
                        }],
                        }],
                    },
                },
            },
        }))?;

        // Create the service.
        let service_api: Api<Service> = Api::default_namespaced(self.client.clone());
        service_api.create(&Default::default(), &service).await?;

        // Create the deployment.
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());
        deployment_api
            .create(&Default::default(), &deployment)
            .await?;

        Ok(App {
            id: unique_id as u64,
            config,
        })
    }

    async fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        // Starting an app means scaling up the deployment to 1 replica.

        let deployment = self.get_deployment(id).await?;
        let name = deployment.metadata.name.ok_or(BackendError::NotFound)?;
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());

        let patch = Patch::Merge(serde_json::json!({
            "spec": {
                "replicas": 1,
            },
        }));
        deployment_api
            .patch(&name, &Default::default(), &patch)
            .await?;

        Ok(AppStatus::Running)
    }

    async fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        // Stopping an app means scaling down the deployment to 0 replicas.

        let deployment = self.get_deployment(id).await?;
        let name = deployment.metadata.name.ok_or(BackendError::NotFound)?;
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());

        let patch = Patch::Merge(serde_json::json!({
            "spec": {
                "replicas": 0,
            },
        }));
        deployment_api
            .patch(&name, &Default::default(), &patch)
            .await?;

        Ok(AppStatus::Stopped)
    }

    async fn delete_app(&self, id: AppId) -> Result<(), BackendError> {
        // Deleting an app means deleting the service and deployment.

        let deployment = self.get_deployment(id).await?;
        let name = deployment.metadata.name.ok_or(BackendError::NotFound)?;
        let service_api: Api<Service> = Api::default_namespaced(self.client.clone());
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());

        service_api.delete(&name, &Default::default()).await?;
        deployment_api.delete(&name, &Default::default()).await?;

        Ok(())
    }

    async fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        let deployment = self.get_deployment(id).await?;
        let labels = deployment.metadata.labels.ok_or(BackendError::NotFound)?;
        let config = AppConfig {
            name: labels["app-controller-name"].to_string(),
            interaction_model: InteractionModel::from_str(
                &labels["app-controller-interaction-model"],
            )
            .map_err(|e| BackendError::InternalError(e.to_string()))?,
            image: labels["app-controller-image"].to_string(),
        };

        Ok(App { id, config })
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());
        let list = deployment_api.list(&Default::default()).await?;

        let apps = list
            .items
            .into_iter()
            .filter_map(|deployment| {
                let labels = deployment.metadata.labels?;
                let id = labels["app-controller-id"].parse().ok()?;
                let config = AppConfig {
                    name: labels["app-controller-name"].to_string(),
                    interaction_model: InteractionModel::parse_from_parameter(
                        &labels["app-controller-interaction-model"],
                    )
                    .ok()?,
                    image: labels["app-controller-image"].to_string(),
                };
                Some(App { id, config })
            })
            .collect();

        Ok(apps)
    }
}
