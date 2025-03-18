use poem_openapi::{types::Example, Enum, Object};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use strum::{Display, EnumString};

/// Image pull policy for containers. Determines when to pull the container image.
#[derive(Debug, Display, Clone, Default, Enum, EnumString, Serialize, Deserialize)]
pub enum ImagePullPolicy {
    /// Always pull the image.
    Always = 0,
    /// Only pull the image if it's not already present.
    #[default]
    IfNotPresent = 1,
    /// Never pull the image, use the local image only.
    Never = 2,
}

/// Interaction model used by the app. The idea here is to allow different ways
/// of running and presenting an interactive app. For now, only X11 is
/// supported.
#[derive(Debug, Display, Clone, Default, Enum, EnumString)]
pub enum InteractionModel {
    /// Unknown interaction model.
    #[default]
    Unknown = 0,
    /// X11 interaction model. App container will be spawned with access to an X
    /// socket to render its output.
    X11 = 1,
}

/// Configuration for a container.
#[derive(Debug, Clone, Default, Object, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Container image to use.
    pub image: String,
    /// Optional configuration data for this container. Limited to 1MB in size.
    /// If provided, this will be mounted into the container and the AC_CONTAINER_CONFIG
    /// environment variable will be set pointing to the mount location.
    #[oai(default)]
    pub config: Option<String>,
    /// Image pull policy for this container. Determines when to pull the container image.
    /// Supports the same values as Kubernetes: Always, IfNotPresent, Never.
    #[oai(default)]
    pub image_pull_policy: Option<ImagePullPolicy>,
}

/// Internal representation of a container specification that supports both simple image names
/// and full container configs. This is used for processing but not directly exposed in the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContainerSpec {
    /// Just an image name (backward compatibility).
    Image(String),
    /// Full container configuration.
    Config(ContainerConfig),
}

impl ContainerSpec {
    /// Get the image name from this container spec
    pub fn image(&self) -> &str {
        match self {
            ContainerSpec::Image(image) => image,
            ContainerSpec::Config(config) => &config.image,
        }
    }

    /// Get the config from this container spec, if any
    pub fn config(&self) -> Option<&str> {
        match self {
            ContainerSpec::Image(_) => None,
            ContainerSpec::Config(config) => config.config.as_deref(),
        }
    }

    /// Get the image pull policy from this container spec, if any
    pub fn image_pull_policy(&self) -> Option<ImagePullPolicy> {
        match self {
            ContainerSpec::Image(_) => None,
            ContainerSpec::Config(config) => config.image_pull_policy.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Object)]
#[oai(example)]
pub struct AppConfig {
    /// Name of the app.
    pub name: String,
    /// Interaction model to use for the app.
    pub interaction_model: InteractionModel,
    /// Container images to use for the app (simple format).
    /// Use either this field or 'containers', not both.
    #[oai(default)]
    pub images: Vec<String>,
    /// Container configurations (advanced format).
    /// Use either this field or 'images', not both.
    #[oai(default)]
    pub containers: Vec<ContainerConfig>,
    /// **Deprecated:** Use container-specific image_pull_policy instead.
    /// This field is maintained for backward compatibility but container-specific
    /// image_pull_policy takes precedence when specified.
    ///
    /// Whether to always pull images from the registry.
    #[oai(default)]
    pub always_pull_images: bool,
    /// Whether to enable Docker socket for this app.
    /// If enabled, a Docker daemon sidecar container will be added
    /// and the Docker socket will be mounted into all containers.
    #[oai(default)]
    pub enable_docker: bool,
    /// Whether to automatically start the app when it's created.
    /// If true, the app will be started immediately after creation.
    /// If false, the app will be created in a stopped state.
    #[oai(default)]
    pub autostart: bool,
}

impl AppConfig {
    /// Get the containers for this app, converting from the API representation
    /// to the internal ContainerSpec representation.
    pub fn get_containers(&self) -> Vec<ContainerSpec> {
        // If containers is provided, use that
        if !self.containers.is_empty() {
            return self
                .containers
                .iter()
                .map(|c| ContainerSpec::Config(c.clone()))
                .collect();
        }

        // Otherwise, use images (convert to container specs)
        self.images
            .iter()
            .map(|i| ContainerSpec::Image(i.clone()))
            .collect()
    }

    /// Get container configs as a map from container index to config
    pub fn get_container_configs(&self) -> BTreeMap<usize, String> {
        let mut configs = BTreeMap::new();

        // Add container-specific configs
        for (i, container) in self.get_containers().iter().enumerate() {
            if let Some(config) = container.config() {
                configs.insert(i, config.to_string());
            }
        }

        configs
    }
}

impl Example for AppConfig {
    fn example() -> Self {
        Self {
            name: "firefox-demo".to_string(),
            interaction_model: InteractionModel::X11,
            images: Vec::new(),
            containers: vec![ContainerConfig {
                image: "ghcr.io/twizmwazin/app-controller/firefox-demo:latest".to_string(),
                config: Some("Container-specific configuration".to_string()),
                image_pull_policy: Some(ImagePullPolicy::IfNotPresent),
            }],
            always_pull_images: false,
            enable_docker: false,
            autostart: false,
        }
    }
}

/// Status of a app.
#[derive(Debug, Clone, Default, Enum)]
pub enum AppStatus {
    #[default]
    Unknown = 0,
    Running = 1,
    Stopped = 2,
}

/// ID used to identify a app in requests.
pub type AppId = u64;

/// Primary object representing a app.
#[derive(Debug, Clone, Default, Object)]
#[oai(read_only_all = true)]
pub struct App {
    /// Unique identifier for the app.
    pub id: AppId,
    /// Current status of the app.
    pub config: AppConfig,
}

pub type IpAddr = String;

/// Address and port of a app.
#[derive(Debug, Clone, Object)]
#[oai(read_only_all = true)]
pub struct SocketAddr {
    pub ip: IpAddr,
    pub port: u16,
}
