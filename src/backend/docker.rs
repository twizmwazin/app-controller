use super::{AppControllerBackend, BackendError};
use crate::types::{
    App, AppConfig, AppId, AppStatus, ContainerConfig, ContainerIndex, ContainerOutput, ImagePullPolicy
};
use bollard::Docker;
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, ContainerSummaryStateEnum, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, InspectContainerOptions, ListContainersOptions,
    LogsOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
    WaitContainerOptions,
};
use bollard::secret::ContainerSummary;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

static X11_HOST_IMAGE: &str = "ghcr.io/twizmwazin/app-controller/x11-host";
static DOCKER_DIND_IMAGE: &str = "docker:dind";

// Label constants
const LABEL_ID: &str = "app-controller-id";
const LABEL_NAME: &str = "app-controller-name";
const LABEL_INTERACTION_MODEL: &str = "app-controller-interaction-model";
const LABEL_CONTAINER_TYPE: &str = "app-controller-container-type";
const LABEL_CONTAINER_INDEX: &str = "app-controller-container-index";
const LABEL_AUTOSTART: &str = "app-controller-autostart";
const LABEL_ENABLE_DOCKER: &str = "app-controller-enable-docker";

// Container type constants
const CONTAINER_TYPE_X11_HOST: &str = "x11-host";
const CONTAINER_TYPE_DOCKER_DAEMON: &str = "docker-daemon";
const CONTAINER_TYPE_APPLICATION: &str = "application";

// Port constants
const VNC_TCP_PORT: u16 = 5910;
const VNC_WS_PORT: u16 = 5911;
const DOCKER_DAEMON_PORT: u16 = 2375;

/// DockerBackend implements the app controller backend using Docker containers.
///
/// # Docker Resources
///
/// Each app is represented by multiple Docker containers that are created and managed
/// together, similar to how Kubernetes manages containers in a pod. The architecture
/// includes infrastructure containers (X11 host, optional Docker daemon) and
/// application containers, all sharing the same network namespace.
///
/// # Container Architecture
///
/// Each app consists of:
/// - An X11 host container: Provides X11 display server and VNC access (ports 5910, 5911)
/// - Optional Docker daemon container: Provides Docker-in-Docker functionality if enabled
/// - Application containers: The actual workload containers specified in the app config
///
/// All containers share the network namespace of the X11 host container, enabling
/// network communication between containers similar to Kubernetes pods.
///
/// # Data Storage
///
/// App data is stored in Docker as follows:
///
/// ## Labels:
/// - `app-controller-id`: Uniquely identifies the app (random 32-bit integer as string)
/// - `app-controller-name`: The app name
/// - `app-controller-interaction-model`: The app's interaction model
/// - `app-controller-container-type`: Type of container (x11-host, docker-daemon, or application)
/// - `app-controller-container-index`: Index of the container within the app (for application containers)
/// - `app-controller-autostart`: "true" or "false" to control whether the app starts automatically upon creation
/// - `app-controller-enable-docker`: "true" or "false" to control Docker sidecar availability
/// - `app-controller-container-{index}-image`: Image for container at index
/// - `app-controller-container-{index}-config`: Config for container at index (if any)
///
/// ## Environment Variables:
/// - `AC_CONTAINER_CONFIG`: Path to container-specific configuration file
/// - `DISPLAY`: X11 display variable (set to ":0.0")
/// - `AC_CONTAINER_OUTPUT`: Path to container output log file
/// - `DOCKER_HOST`: Docker daemon endpoint (if Docker-in-Docker is enabled)
///
/// ## Volumes:
/// - Shared volume for container outputs
/// - Individual volumes for container-specific configurations
///
/// ## App State:
/// - App state (running/stopped) is determined by the running state of the infrastructure containers:
///   - Infrastructure containers stopped = app stopped
///   - Infrastructure containers running = app ready
pub struct DockerBackend {
    docker: Arc<Docker>,
}

impl DockerBackend {
    pub fn new(docker: Docker) -> Self {
        Self {
            docker: Arc::new(docker),
        }
    }
}

impl Default for DockerBackend {
    fn default() -> Self {
        Self::new(
            Docker::connect_with_local_defaults().expect("Failed to connect to Docker daemon"),
        )
    }
}

#[async_trait::async_trait]
impl AppControllerBackend for DockerBackend {
    async fn create_app(&self, config: AppConfig) -> Result<App, BackendError> {
        let docker = self.docker.clone();
        let unique_id: u32 = rand::random();
        let app_name = format!("{}-{:08x}", config.name, unique_id);

        // Create base labels for all containers in this app
        let mut base_labels = Self::create_base_labels(&config, unique_id);

        // Get the containers for this app
        let containers = config.get_containers();

        // Add container-specific labels for reconstruction
        for (i, container) in containers.iter().enumerate() {
            base_labels.insert(
                format!("app-controller-container-{i}-image"),
                container.image().to_string(),
            );

            if let Some(config_data) = container.config() {
                base_labels.insert(
                    format!("app-controller-container-{i}-config"),
                    config_data.to_string(),
                );
            }
        }

        // Get container configs as a map from container index to config
        let container_configs = config.get_container_configs();

        // Create a shared volume for container outputs
        let output_volume_name = format!("{app_name}-outputs");

        // Create X11 host container first
        let x11_container_id = self.create_x11_container(&app_name, &base_labels).await?;
        let x11_container_name = format!("{app_name}-x11-host");
        let mut created_containers = vec![x11_container_id];

        // Create Docker daemon container if Docker is enabled
        if config.enable_docker {
            if let Some(docker_container_id) = self
                .create_docker_daemon_container(&app_name, &x11_container_name, &base_labels)
                .await?
            {
                created_containers.push(docker_container_id);
            }
        }

        // Create application containers
        for (index, container_spec) in containers.iter().enumerate() {
            let image = container_spec.image();
            let should_pull = match container_spec.image_pull_policy() {
                Some(ImagePullPolicy::Always) => true,
                Some(ImagePullPolicy::Never) => false,
                Some(ImagePullPolicy::IfNotPresent) | None => {
                    // Use app-level setting if container doesn't specify
                    if container_spec.image_pull_policy().is_none() {
                        config.always_pull_images
                    } else {
                        true
                    }
                }
            };
            if should_pull {
                self.pull_image(image).await?;
            }

            // Prepare environment variables
            let mut env_vars = vec![
                "DISPLAY=:0.0".to_string(),
                format!("AC_CONTAINER_OUTPUT=/outputs/container-{}.log", index),
            ];

            // Add AC_CONTAINER_CONFIG env var if this container has config
            if container_configs.contains_key(&index) {
                env_vars.push("AC_CONTAINER_CONFIG=/etc/app-controller/config/config".to_string());
            }

            // Add docker host env var if Docker is enabled
            if config.enable_docker {
                env_vars.push(format!("DOCKER_HOST=tcp://127.0.0.1:{DOCKER_DAEMON_PORT}"));
            }

            let mut app_labels = base_labels.clone();
            app_labels.insert(
                LABEL_CONTAINER_TYPE.to_string(),
                CONTAINER_TYPE_APPLICATION.to_string(),
            );
            app_labels.insert(LABEL_CONTAINER_INDEX.to_string(), index.to_string());

            // Configure host settings to share network with X11 container
            let mut host_config = HostConfig {
                network_mode: Some(format!("container:{x11_container_name}")),
                ..Default::default()
            };

            // Add volumes for configuration and outputs
            let mut binds = vec![format!("{}:/outputs", output_volume_name)];

            // Add config volume if this container has config
            if let Some(config_data) = container_configs.get(&index) {
                let config_volume_name = format!("{app_name}-container-{index}-config");

                // Create a temporary container to write the config file
                let temp_container = docker
                    .create_container(
                        Some(CreateContainerOptions {
                            name: Some(format!("{config_volume_name}-temp-config")),
                            ..Default::default()
                        }),
                        ContainerCreateBody {
                            image: Some("alpine:latest".to_string()),
                            // Use a more secure approach with environment variables
                            env: Some(vec![format!("CONFIG_DATA={config_data}")]),
                            cmd: Some(vec![
                                "sh".to_string(),
                                "-c".to_string(),
                                "echo \"$CONFIG_DATA\" > /config/config".to_string(),
                            ]),
                            host_config: Some(HostConfig {
                                binds: Some(vec![format!("{config_volume_name}:/config")]),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )
                    .await?;

                // Run the temp container to create the config file
                docker
                    .start_container(&temp_container.id, None::<StartContainerOptions>)
                    .await?;

                // Wait for it to finish and remove it
                docker
                    .wait_container(&temp_container.id, None::<WaitContainerOptions>)
                    .next()
                    .await;

                docker
                    .remove_container(
                        &temp_container.id,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await?;

                binds.push(format!("{config_volume_name}:/etc/app-controller/config"));
            }

            host_config.binds = Some(binds);

            let container_name = format!("{app_name}-app-{index}");
            let create_opts = ContainerCreateBody {
                image: Some(image.to_string()),
                env: Some(env_vars),
                labels: Some(app_labels),
                host_config: Some(host_config),
                tty: Some(container_spec.tty()),
                ..Default::default()
            };

            let app_container = docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: Some(container_name),
                        ..Default::default()
                    }),
                    create_opts,
                )
                .await?;

            created_containers.push(app_container.id);
        }

        // Start containers if autostart is enabled
        if config.autostart {
            self.start_containers(&created_containers).await?;
        }

        Ok(App {
            id: unique_id as u64,
            config,
        })
    }

    async fn start_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        let containers = self.get_app_containers(id).await?;
        let (x11_containers, docker_containers, app_containers) =
            Self::categorize_containers(&containers);

        // Start infrastructure containers first
        self.start_containers(&x11_containers).await?;
        self.start_containers(&docker_containers).await?;

        // Then start application containers
        let app_container_ids: Vec<String> = app_containers.into_iter().map(|(id, _)| id).collect();
        self.start_containers(&app_container_ids).await?;

        Ok(AppStatus::Ready)
    }

    async fn stop_app(&self, id: AppId) -> Result<AppStatus, BackendError> {
        let containers = self.get_app_containers(id).await?;
        let all_ids = containers
            .iter()
            .filter_map(|c| c.id.clone())
            .collect::<Vec<_>>();
        self.stop_containers(&all_ids).await?;
        Ok(AppStatus::Stopped)
    }

    async fn delete_app(&self, id: AppId) -> Result<(), BackendError> {
        let docker = self.docker.clone();
        let filters = Self::create_app_filters(id);

        let containers = docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            }))
            .await?;

        // Remove all containers
        for container in containers {
            if let Some(container_id) = container.id {
                let _ = docker
                    .remove_container(
                        &container_id,
                        Some(RemoveContainerOptions {
                            force: true,
                            v: true, // Remove associated volumes
                            ..Default::default()
                        }),
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn get_app_status(&self, id: AppId) -> Result<AppStatus, BackendError> {
        let containers = self.get_app_containers(id).await?;
        if containers.is_empty() {
            return Ok(AppStatus::Stopped);
        }
        let mut x11_running = false;
        for container in containers {
            if let (Some(state), Some(labels)) = (&container.state, &container.labels) {
                if state == &ContainerSummaryStateEnum::RUNNING {
                    if let Some(container_type) = labels.get(LABEL_CONTAINER_TYPE) {
                        if container_type == CONTAINER_TYPE_X11_HOST {
                            x11_running = true;
                            break;
                        }
                    }
                }
            }
        }
        Ok(if x11_running {
            AppStatus::Ready
        } else {
            AppStatus::Stopped
        })
    }

    async fn get_app(&self, id: AppId) -> Result<App, BackendError> {
        let containers = self.get_app_containers(id).await?;
        if containers.is_empty() {
            return Err(BackendError::NotFound);
        }
        let first_container =
            containers
                .iter()
                .find(|c| c.labels.is_some())
                .ok_or(BackendError::InternalError(format!(
                    "No containers with labels found for app ID {id}"
                )))?;
        let labels = first_container.labels.as_ref().unwrap();
        DockerBackend::make_app_config_from_labels(labels, id).ok_or(BackendError::InternalError(
            format!("Failed to reconstruct AppConfig from labels for app ID {id}"),
        ))
    }

    async fn get_all_apps(&self) -> Result<Vec<App>, BackendError> {
        let mut filters = HashMap::new();
        filters.insert("label".to_string(), vec![LABEL_ID.to_string()]);

        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            }))
            .await?;

        // Collect unique apps by ID, reconstructing each app from its labels
        let apps: HashMap<u64, App> = containers
            .into_iter()
            .filter_map(|container| {
                let labels = container.labels.as_ref()?;
                let app_id_str = labels.get(LABEL_ID)?;
                let app_id = app_id_str.parse::<u64>().ok()?;
                Self::make_app_config_from_labels(labels, app_id).map(|app| (app_id, app))
            })
            .collect();

        Ok(apps.into_values().collect())
    }

    async fn get_app_addr(&self, id: AppId) -> Result<(IpAddr, u16), BackendError> {
        let docker = self.docker.clone();
        let filters = Self::create_app_filters_with_labels(
            id,
            &[(LABEL_CONTAINER_TYPE, CONTAINER_TYPE_X11_HOST)],
        );

        let containers = docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            }))
            .await?;

        let x11_container = containers
            .into_iter()
            .next()
            .ok_or(BackendError::NotFound)?;

        if let Some(container_id) = &x11_container.id {
            // Inspect the container to get its IP address
            let inspect_result = docker
                .inspect_container(container_id, None::<InspectContainerOptions>)
                .await?;

            if let Some(network_settings) = inspect_result.network_settings {
                if let Some(networks) = network_settings.networks {
                    // Get the first network's IP address
                    for (_, network) in networks {
                        if let Some(ip_str) = network.ip_address {
                            if !ip_str.is_empty() {
                                let ip = ip_str.parse::<IpAddr>()?;
                                return Ok((ip, VNC_WS_PORT)); // VNC WebSocket port
                            }
                        }
                    }
                }
            }
        }

        Err(BackendError::NotFound)
    }

    async fn get_app_output(
        &self,
        id: AppId,
        container_index: ContainerIndex,
    ) -> Result<ContainerOutput, BackendError> {
        let docker = self.docker.clone();
        let filters = Self::create_app_filters_with_labels(
            id,
            &[
                (LABEL_CONTAINER_TYPE, CONTAINER_TYPE_APPLICATION),
                (LABEL_CONTAINER_INDEX, &container_index.to_string()),
            ],
        );

        let containers = docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            }))
            .await?;

        if let Some(container) = containers.first() {
            if let Some(container_id) = &container.id {
                let mut logs = docker.logs(
                    container_id,
                    Some(LogsOptions {
                        stdout: true,
                        stderr: true,
                        follow: false,
                        ..Default::default()
                    }),
                );
                let mut output = String::new();
                while let Some(log) = logs.next().await {
                    let log = log?;
                    match log {
                        LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                            output.push_str(&String::from_utf8_lossy(&message));
                        }
                        _ => {}
                    }
                }
                return Ok(output);
            }
        }

        Err(BackendError::NotFound)
    }
}

impl DockerBackend {
    /// Creates base labels for all containers in an app
    fn create_base_labels(config: &AppConfig, unique_id: u32) -> HashMap<String, String> {
        HashMap::from([
            (LABEL_ID.to_string(), unique_id.to_string()),
            (LABEL_NAME.to_string(), config.name.clone()),
            (
                LABEL_INTERACTION_MODEL.to_string(),
                config.interaction_model.to_string(),
            ),
            (LABEL_AUTOSTART.to_string(), config.autostart.to_string()),
            (
                LABEL_ENABLE_DOCKER.to_string(),
                config.enable_docker.to_string(),
            ),
        ])
    }

    /// Creates the X11 host container
    async fn create_x11_container(
        &self,
        app_name: &str,
        base_labels: &HashMap<String, String>,
    ) -> Result<String, BackendError> {
        let x11_image = X11_HOST_IMAGE;
        self.pull_image(x11_image).await?;

        let mut x11_labels = base_labels.clone();
        x11_labels.insert(
            LABEL_CONTAINER_TYPE.to_string(),
            CONTAINER_TYPE_X11_HOST.to_string(),
        );

        let x11_host_config = HostConfig {
            privileged: Some(true),
            ..Default::default()
        };

        let x11_container_name = format!("{app_name}-x11-host");
        let x11_create_opts = ContainerCreateBody {
            image: Some(x11_image.to_string()),
            labels: Some(x11_labels),
            host_config: Some(x11_host_config),
            exposed_ports: Some({
                let mut ports = HashMap::new();
                ports.insert(format!("{VNC_TCP_PORT}/tcp"), Default::default());
                ports.insert(format!("{VNC_WS_PORT}/tcp"), Default::default());
                ports
            }),
            ..Default::default()
        };

        let x11_container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(x11_container_name),
                    ..Default::default()
                }),
                x11_create_opts,
            )
            .await?;

        Ok(x11_container.id)
    }

    /// Creates the Docker daemon container if enabled
    async fn create_docker_daemon_container(
        &self,
        app_name: &str,
        x11_container_name: &str,
        base_labels: &HashMap<String, String>,
    ) -> Result<Option<String>, BackendError> {
        let docker_image = DOCKER_DIND_IMAGE;
        self.pull_image(docker_image).await?;

        let mut docker_labels = base_labels.clone();
        docker_labels.insert(
            LABEL_CONTAINER_TYPE.to_string(),
            CONTAINER_TYPE_DOCKER_DAEMON.to_string(),
        );

        let docker_host_config = HostConfig {
            privileged: Some(true),
            network_mode: Some(format!("container:{x11_container_name}")),
            ..Default::default()
        };

        let docker_container_name = format!("{app_name}-docker-daemon");
        let docker_create_opts = ContainerCreateBody {
            image: Some(docker_image.to_string()),
            labels: Some(docker_labels),
            host_config: Some(docker_host_config),
            cmd: Some(vec![
                "dockerd".to_string(),
                format!("--host=tcp://0.0.0.0:{DOCKER_DAEMON_PORT}"),
                "--tls=false".to_string(),
            ]),
            ..Default::default()
        };

        let docker_container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(docker_container_name),
                    ..Default::default()
                }),
                docker_create_opts,
            )
            .await?;

        Ok(Some(docker_container.id))
    }

    /// Helper to create label filters for Docker API calls
    fn create_app_filters(id: AppId) -> HashMap<String, Vec<String>> {
        let mut filters = HashMap::new();
        filters.insert("label".to_string(), vec![format!("{LABEL_ID}={}", id)]);
        filters
    }

    /// Helper to create label filters with additional constraints
    fn create_app_filters_with_labels(
        id: AppId,
        extra_labels: &[(&str, &str)],
    ) -> HashMap<String, Vec<String>> {
        let mut filters = HashMap::new();
        let mut labels = vec![format!("{LABEL_ID}={}", id)];

        for (key, value) in extra_labels {
            labels.push(format!("{key}={value}"));
        }

        filters.insert("label".to_string(), labels);
        filters
    }

    // Helper to pull an image
    async fn pull_image(&self, image: &str) -> Result<(), BackendError> {
        let mut pull_stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: Some(image.to_string()),
                ..Default::default()
            }),
            None,
            None,
        );
        while pull_stream.next().await.is_some() {}
        Ok(())
    }

    // Helper to start containers by id
    async fn start_containers(&self, container_ids: &[String]) -> Result<(), BackendError> {
        for container_id in container_ids {
            self.docker
                .start_container(container_id, None::<StartContainerOptions>)
                .await?;
        }
        Ok(())
    }

    // Helper to stop containers by id
    async fn stop_containers(&self, container_ids: &[String]) -> Result<(), BackendError> {
        for container_id in container_ids {
            let _ = self
                .docker
                .stop_container(container_id, None::<StopContainerOptions>)
                .await;
        }
        Ok(())
    }

    // Helper to filter and sort containers
    fn categorize_containers(
        containers: &[ContainerSummary],
    ) -> (Vec<String>, Vec<String>, Vec<(String, usize)>) {
        let mut x11 = Vec::new();
        let mut docker_daemon = Vec::new();
        let mut app = Vec::new();
        for container in containers {
            if let (Some(id), Some(labels)) = (&container.id, &container.labels) {
                match labels.get(LABEL_CONTAINER_TYPE).map(|s| s.as_str()) {
                    Some(CONTAINER_TYPE_X11_HOST) => x11.push(id.clone()),
                    Some(CONTAINER_TYPE_DOCKER_DAEMON) => docker_daemon.push(id.clone()),
                    Some(CONTAINER_TYPE_APPLICATION) => app.push((
                        id.clone(),
                        labels
                            .get(LABEL_CONTAINER_INDEX)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0),
                    )),
                    _ => {}
                }
            }
        }
        app.sort_by_key(|(_, idx)| *idx);
        (x11, docker_daemon, app)
    }

    // Helper to reconstruct AppConfig from labels
    fn make_app_config_from_labels(labels: &HashMap<String, String>, id: AppId) -> Option<App> {
        let name = labels.get(LABEL_NAME)?.clone();
        let interaction_model = labels
            .get(LABEL_INTERACTION_MODEL)
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        let autostart = labels
            .get(LABEL_AUTOSTART)
            .map(|s| s == "true")
            .unwrap_or(false);
        let enable_docker = labels
            .get(LABEL_ENABLE_DOCKER)
            .map(|s| s == "true")
            .unwrap_or(false);
        let mut container_configs = Vec::new();
        let mut index = 0;
        while let Some(image) = labels.get(&format!("app-controller-container-{index}-image")) {
            let config = labels.get(&format!("app-controller-container-{index}-config"));
            container_configs.push(ContainerConfig {
                image: image.clone(),
                config: config.cloned(),
                image_pull_policy: None,
                tty: false,
            });
            index += 1;
        }
        let config = AppConfig {
            name,
            interaction_model,
            images: Vec::new(),
            containers: container_configs,
            always_pull_images: false,
            enable_docker,
            autostart,
        };
        Some(App { id, config })
    }

    /// Helper to get all containers for a given app id
    async fn get_app_containers(
        &self,
        id: AppId,
    ) -> Result<Vec<bollard::models::ContainerSummary>, BackendError> {
        let filters = Self::create_app_filters(id);
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            }))
            .await?;
        Ok(containers)
    }
}
