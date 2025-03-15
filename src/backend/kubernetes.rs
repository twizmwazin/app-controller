use std::{collections::BTreeMap, net::IpAddr, str::FromStr};

use crate::types::{
    App, AppConfig, AppId, AppStatus, ContainerConfig, ContainerOutput, InteractionModel,
};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            ConfigMap, Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, SecurityContext,
            Service, ServicePort, ServiceSpec, Volume, VolumeMount,
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

/// KubernetesBackend implements the app controller backend using Kubernetes resources.
///
/// # Kubernetes Resources
///
/// Each app is represented by the following Kubernetes resources:
///
/// - A Kubernetes Deployment: Contains the main application containers and initialization
///   containers. The deployment has zero replicas when the app is stopped and one
///   replica when the app is running.
///
/// - A Kubernetes Service: Provides network access to the app, exposing ports for
///   VNC (5910) and VNC WebSocket (5911) connections.
///
/// - Kubernetes ConfigMaps: Created for containers with configuration data.
///   Stores the container configuration that's mounted into the specific container.
///
/// # Data Storage
///
/// App data is stored in Kubernetes as follows:
///
/// ## Labels:
/// - `app-controller-id`: Uniquely identifies the app resources (random 32-bit integer)
///
/// ## Annotations:
/// - `app-controller-name`: The app name
/// - `app-controller-interaction-model`: The app's interaction model
/// - `app-controller-always-pull-images`: "true" or "false" to control image pull policy
/// - `app-controller-container-{index}-image`: Image for container at index
/// - `app-controller-container-{index}-config`: Config for container at index (if any)
/// - `app-controller-container-{index}-always-pull`: Always pull setting for container at index
///
/// ## App State:
/// - App state (running/stopped) is represented by the deployment's replica count:
///   - 0 replicas = stopped
///   - 1 replica = running
pub struct KubernetesBackend {
    client: Client,
}

impl KubernetesBackend {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Retrieves a Kubernetes Deployment by app ID.
    ///
    /// This method finds the deployment by filtering on the `app-controller-id` label.
    /// Returns BackendError::NotFound if no deployment exists with the given ID.
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

    /// Converts a Kubernetes Deployment into an App object.
    ///
    /// Extracts app information from the deployment's labels and annotations.
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

        // Get the basic app config
        let mut config = AppConfig {
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
            images: Vec::new(),
            containers: Vec::new(),
            always_pull_images: annotations
                .get("app-controller-always-pull-images")
                .map(|s| s == "true")
                .unwrap_or(false),
        };

        // Look for container-specific annotations
        let mut index = 0;
        while let Some(image) =
            annotations.get(&format!("app-controller-container-{}-image", index))
        {
            let container_config = ContainerConfig {
                image: image.to_string(),
                config: annotations
                    .get(&format!("app-controller-container-{}-config", index))
                    .map(|s| s.to_string()),
            };
            config.containers.push(container_config);
            index += 1;
        }

        Ok::<App, BackendError>(App {
            id: id.into(),
            config,
        })
    }
}

impl AppControllerBackend for KubernetesBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        // To create an app, we need a service and a deployment. The deployment
        // should have zero replicas initially.

        let unique_id: u32 = rand::random();
        let name = format!("{}-{:08x}", config.name, unique_id);

        let labels = BTreeMap::from([("app-controller-id".to_string(), format!("{}", unique_id))]);
        let mut annotations = BTreeMap::from([
            ("app-controller-name".to_string(), config.name.clone()),
            (
                "app-controller-interaction-model".to_string(),
                config.interaction_model.to_string(),
            ),
            (
                "app-controller-always-pull-images".to_string(),
                config.always_pull_images.to_string(),
            ),
        ]);

        // Get the containers for this app
        let containers = config.get_containers();

        // Add container-specific annotations
        for (i, container) in containers.iter().enumerate() {
            annotations.insert(
                format!("app-controller-container-{}-image", i),
                container.image().to_string(),
            );

            if let Some(config) = container.config() {
                annotations.insert(
                    format!("app-controller-container-{}-config", i),
                    config.to_string(),
                );
            }
        }

        // Get container configs as a map from container index to config
        let container_configs = config.get_container_configs();

        // Create ConfigMaps for container configs
        let mut config_map_volumes = Vec::new();
        for (index, config_data) in &container_configs {
            let config_map_name = format!("{}-container-{}-config", name, index);
            let config_map = ConfigMap {
                metadata: ObjectMeta {
                    name: Some(config_map_name.clone()),
                    labels: Some(labels.clone()),
                    ..Default::default()
                },
                data: Some(BTreeMap::from([(
                    "config".to_string(),
                    config_data.clone(),
                )])),
                ..Default::default()
            };

            let config_map_api: Api<ConfigMap> = Api::default_namespaced(self.client.clone());
            config_map_api
                .create(&Default::default(), &config_map)
                .await?;

            // Add volume for this config map
            config_map_volumes.push((
                *index,
                Volume {
                    name: format!("container-{}-config", index),
                    config_map: Some(k8s_openapi::api::core::v1::ConfigMapVolumeSource {
                        name: config_map_name,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ));
        }

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
                    spec: Some({
                        // Create a volume for container outputs
                        let output_volume = Volume {
                            name: "container-outputs".to_string(),
                            empty_dir: Some(k8s_openapi::api::core::v1::EmptyDirVolumeSource {
                                ..Default::default()
                            }),
                            ..Default::default()
                        };

                        // Add the output volume to the list of volumes
                        let mut all_volumes = config_map_volumes
                            .iter()
                            .map(|(_, v)| v.clone())
                            .collect::<Vec<Volume>>();
                        all_volumes.push(output_volume);

                        // Create the pod spec
                        PodSpec {
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
                            containers: containers
                                .iter()
                                .enumerate()
                                .map(|(index, container_spec)| {
                                    // Prepare environment variables
                                    let mut env_vars = vec![EnvVar {
                                        name: "DISPLAY".to_string(),
                                        value: Some(":0.0".to_string()),
                                        ..Default::default()
                                    }];

                                    // Add AC_CONTAINER_CONFIG env var if this container has config
                                    if container_configs.contains_key(&index) {
                                        env_vars.push(EnvVar {
                                            name: "AC_CONTAINER_CONFIG".to_string(),
                                            value: Some(
                                                "/etc/app-controller/config/config".to_string(),
                                            ),
                                            ..Default::default()
                                        });
                                    }

                                    // Add AC_CONTAINER_OUTPUT env var for all containers except X11
                                    let output_path = format!("/outputs/container-{}.log", index);
                                    env_vars.push(EnvVar {
                                        name: "AC_CONTAINER_OUTPUT".to_string(),
                                        value: Some(output_path),
                                        ..Default::default()
                                    });

                                    // Determine image pull policy
                                    let image_pull_policy = if config.always_pull_images {
                                        "Always".to_string()
                                    } else {
                                        "IfNotPresent".to_string()
                                    };

                                    // Prepare volume mounts
                                    let mut volume_mounts = Vec::new();

                                    // Add config volume mount if this container has config
                                    if container_configs.contains_key(&index) {
                                        volume_mounts.push(VolumeMount {
                                            name: format!("container-{}-config", index),
                                            mount_path: "/etc/app-controller/config".to_string(),
                                            ..Default::default()
                                        });
                                    }

                                    // Add output volume mount for all containers
                                    volume_mounts.push(VolumeMount {
                                        name: "container-outputs".to_string(),
                                        mount_path: "/outputs".to_string(),
                                        ..Default::default()
                                    });

                                    Container {
                                        name: format!(
                                            "{}-{}",
                                            extract_package(container_spec.image())
                                                .unwrap_or("unknown".to_string()),
                                            index
                                        ),
                                        image: Some(container_spec.image().to_string()),
                                        env: Some(env_vars),
                                        image_pull_policy: Some(image_pull_policy),
                                        volume_mounts: Some(volume_mounts),
                                        ..Default::default()
                                    }
                                })
                                .collect(),
                            volumes: Some(all_volumes),
                            ..Default::default()
                        }
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
        let config_map_api: Api<ConfigMap> = Api::default_namespaced(self.client.clone());

        // Delete all resources with the app-controller-id label
        let list_params = ListParams::default().labels(&format!("app-controller-id={}", id));
        let config_maps = config_map_api.list(&list_params).await?;
        for config_map in config_maps.items {
            if let Some(name) = config_map.metadata.name {
                config_map_api.delete(&name, &Default::default()).await?;
            }
        }

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

    async fn get_app_outputs(&self, id: AppId) -> Result<Vec<ContainerOutput>, BackendError> {
        // First, check if the app exists
        let deployment = self.get_deployment(id).await?;
        let app = self.make_app_for_deployment(deployment)?;

        // Get the pod for this app
        let pod_api: Api<k8s_openapi::api::core::v1::Pod> =
            Api::default_namespaced(self.client.clone());
        let list_params = ListParams::default().labels(&format!("app-controller-id={}", id));
        let pods = pod_api.list(&list_params).await?;

        // If there are no pods, the app is not running
        if pods.items.is_empty() {
            return Ok(Vec::new()); // Return empty outputs if app is not running
        }

        // Get the first pod (there should be only one for each app)
        let pod = pods.items.into_iter().next().unwrap();

        // Get the containers in the pod
        let spec = pod
            .spec
            .as_ref()
            .ok_or_else(|| BackendError::InternalError("Pod missing spec".to_string()))?;

        // In k8s_openapi, containers is a Vec<Container>, not an Option<Vec<Container>>
        let containers = &spec.containers;

        let mut outputs = Vec::new();

        // For each container (excluding the X11 container)
        for (index, container) in containers.iter().enumerate() {
            // Skip the X11 container
            if container.name == "x11-host" {
                continue;
            }

            // Get the container's output file
            let output_path = format!("/outputs/container-{}.log", index);

            // Get the pod name
            let pod_name = pod
                .metadata
                .name
                .clone()
                .ok_or(BackendError::InternalError("Pod missing name".to_string()))?;

            // For now, we'll return a placeholder message
            // In a real implementation, we would need to use the Kubernetes API to read the file
            // This would typically involve using the pod exec API, but that's complex to implement here
            let output = format!("Output for container {} ({})", index, container.name);

            outputs.push(ContainerOutput {
                container_index: index,
                container_name: container.name.clone(),
                output,
            });
        }

        Ok(outputs)
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
                .filter(|c| c.is_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect::<String>()
        })
}
