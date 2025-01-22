use std::{collections::BTreeMap, net::IpAddr, str::FromStr};

use crate::types::{App, AppConfig, AppId, AppStatus, InteractionModel};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, SecurityContext, Service,
            ServicePort, ServiceSpec,
        },
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{
    api::{ListParams, ObjectMeta, Patch},
    Api, Client,
};
use poem_openapi::types::ParseFromParameter;

use super::{AppControllerBackend, BackendError};

static X11_HOST_IMAGE: &str = "ghcr.io/twizmwazin/app-controller/x11-host";

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

    fn make_app_for_deployment(&self, deployment: Deployment) -> Result<App, BackendError> {
        let name = deployment.metadata.name.unwrap_or("unknown".to_string());
        let id: u32 = deployment
            .metadata
            .labels
            .ok_or(BackendError::InternalError(format!(
                "Deployment {} missing labels",
                name
            )))?
            .get("app-controller-id")
            .ok_or(BackendError::InternalError(format!(
                "Deployment {} missing app-controller-id",
                name
            )))?
            .parse()
            .map_err(|_| {
                BackendError::InternalError(format!(
                    "Invalid app-controller-id label for deployment {}",
                    name
                ))
            })?;

        let annotations = deployment
            .metadata
            .annotations
            .ok_or(BackendError::InternalError(format!(
                "Deployment {} missing annotations",
                name
            )))?;

        let config = AppConfig {
            name: annotations
                .get("app-controller-name")
                .ok_or(BackendError::InternalError(format!(
                    "Missing app-controller-name annotation for deployment {}",
                    name
                )))?
                .to_string(),
            interaction_model: InteractionModel::parse_from_parameter(
                annotations.get("app-controller-interaction-model").ok_or(
                    BackendError::InternalError(format!(
                        "Missing app-controller-interaction-model annotation for deployment {}",
                        name
                    )),
                )?,
            )?,
            images: annotations
                .get("app-controller-images")
                .ok_or(BackendError::InternalError(format!(
                    "Missing app-controller-images annotation for deployment {}",
                    name
                )))?
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            always_pull_images: annotations
                .get("app-controller-always-pull-images")
                .map(|s| s == "true")
                .unwrap_or(false),
        };
        Ok::<App, BackendError>(App {
            id: id.into(),
            config,
        })
    }
}

impl AppControllerBackend for KubernetesBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        // To creat an app, we need a service and a deployment. The deployment
        // should have zero replicas initially.

        let unique_id: u32 = rand::random();
        let name = format!("{}-{:08x}", config.name, unique_id);

        let labels = BTreeMap::from([("app-controller-id".to_string(), format!("{}", unique_id))]);
        let annotations = BTreeMap::from([
            ("app-controller-name".to_string(), config.name.clone()),
            (
                "app-controller-interaction-model".to_string(),
                config.interaction_model.to_string(),
            ),
            ("app-controller-images".to_string(), config.images.join(",")),
            (
                "app-controller-always-pull-images".to_string(),
                config.always_pull_images.to_string(),
            ),
        ]);

        let service = Service {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                labels: Some(labels.clone()),
                annotations: Some(annotations.clone()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(labels.clone()),
                ports: Some(vec![
                    ServicePort {
                        name: Some("vnc".to_string()),
                        protocol: Some("TCP".to_string()),
                        port: 5910,
                        ..Default::default()
                    },
                    ServicePort {
                        name: Some("vnc-websocket".to_string()),
                        protocol: Some("TCP".to_string()),
                        port: 5911,
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let deployment = Deployment {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                labels: Some(labels.clone()),
                annotations: Some(annotations.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(0),
                selector: LabelSelector {
                    match_labels: Some(labels.clone()),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(labels.clone()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        init_containers: Some(vec![Container {
                            name: "x11-host".to_string(),
                            image: Some(X11_HOST_IMAGE.to_string()),
                            ports: Some(vec![
                                ContainerPort {
                                    container_port: 5910,
                                    ..Default::default()
                                },
                                ContainerPort {
                                    container_port: 5911,
                                    ..Default::default()
                                },
                            ]),
                            restart_policy: Some("Always".to_string()),
                            security_context: Some(SecurityContext {
                                privileged: Some(true),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }]),
                        containers: config
                            .images
                            .iter()
                            .enumerate()
                            .map(|(index, image)| Container {
                                name: format!(
                                    "{}-{}",
                                    extract_package(image).unwrap_or("unknown".to_string()),
                                    index
                                ),
                                image: Some(image.clone()),
                                env: Some(vec![EnvVar {
                                    name: "DISPLAY".to_string(),
                                    value: Some(":0.0".to_string()),
                                    ..Default::default()
                                }]),
                                image_pull_policy: Some(match config.always_pull_images {
                                    true => "Always".to_string(),
                                    false => "IfNotPresent".to_string(),
                                }),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        };

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
        self.make_app_for_deployment(self.get_deployment(id).await?)
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        let deployment_api: Api<Deployment> = Api::default_namespaced(self.client.clone());
        let list = deployment_api
            .list(&ListParams {
                label_selector: Some("app-controller-id".to_string()),
                ..Default::default()
            })
            .await?;

        let apps = list
            .items
            .into_iter()
            .map(|deployment| self.make_app_for_deployment(deployment))
            .collect::<Result<Vec<App>, BackendError>>()?;

        Ok(apps)
    }

    async fn get_app_addr(&self, id: AppId) -> Result<(IpAddr, u16), BackendError> {
        let service_api: Api<Service> = Api::default_namespaced(self.client.clone());
        let list_params = ListParams::default().labels(&format!("app-controller-id={}", id));
        let list = service_api.list(&list_params).await?;

        // There should be only one service with the given ID.
        let service = list
            .items
            .into_iter()
            .next()
            .ok_or(BackendError::NotFound)?;

        // Get the IP address of the service.
        let raw = service
            .spec
            .ok_or(BackendError::InternalError(
                "Missing spec in service".to_string(),
            ))?
            .cluster_ip
            .ok_or(BackendError::InternalError(
                "Missing cluster IP in service".to_string(),
            ))?;

        IpAddr::from_str(&raw)
            .map(|ip| (ip, 5911))
            .map_err(|e| BackendError::InternalError(e.to_string()))
    }
}

fn extract_package(image_name: &str) -> Option<String> {
    image_name
        .rsplit('/')
        .next() // Get the last segment after the last `/`
        .and_then(|segment| segment.split(':').next()) // Split by `:` and get the part before the tag
        .map(|package| {
            package
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
        })
}
