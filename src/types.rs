use poem_openapi::{Enum, Object};
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
pub struct AppConfig {
    /// Name of the app.
    pub name: String,
    /// Interaction model to use for the app.
    pub interaction_model: InteractionModel,
    /// Container image to use for the app.
    pub image: String,
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
