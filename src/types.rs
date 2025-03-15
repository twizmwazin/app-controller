use poem_openapi::{types::Example, Enum, Object};
use strum::{Display, EnumString};

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

#[derive(Debug, Clone, Default, Object)]
#[oai(example)]
pub struct AppConfig {
    /// Name of the app.
    pub name: String,
    /// Interaction model to use for the app.
    pub interaction_model: InteractionModel,
    /// Container images to use for the app.
    pub images: Vec<String>,
    /// Whether to always pull images from the registry.
    #[oai(default)]
    pub always_pull_images: bool,
    /// Optional configuration data for the app. Limited to 1MB in size.
    /// If provided, this will be mounted into all containers and the AC_APP_CONFIG
    /// environment variable will be set pointing to the mount location.
    #[oai(default)]
    pub app_config: Option<String>,
}

impl Example for AppConfig {
    fn example() -> Self {
        Self {
            name: "firefox-demo".to_string(),
            interaction_model: InteractionModel::X11,
            images: vec!["ghcr.io/twizmwazin/app-container/firefox-demo:latest".to_string()],
            always_pull_images: false,
            app_config: Some("# Sample configuration\nkey1: value1\nkey2: value2".to_string()),
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
