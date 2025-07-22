use app_controller::{
    api::Api,
    backend::{AppControllerBackend, DockerBackend, KubernetesBackend, NullBackend},
};
use bollard::Docker;
use kube::Client;
use poem::{Route, Server, get, handler, listener::TcpListener, web::Redirect};
use poem_openapi::OpenApiService;
use std::env;

#[handler]
fn index() -> Redirect {
    Redirect::temporary("/doc")
}

async fn initialize_backend() -> Option<Box<dyn AppControllerBackend>> {
    let backend_type =
        env::var("APP_CONTROLLER_BACKEND").unwrap_or_else(|_| "kubernetes".to_string());
    match backend_type.as_str() {
        "kubernetes" => {
            let k8s_client = Client::try_default().await.ok()?;
            tracing::info!("Using Kubernetes backend");
            Some(Box::new(KubernetesBackend::new(k8s_client)))
        }
        "docker" => {
            let docker_client = Docker::connect_with_local_defaults().ok()?;
            tracing::info!("Using Docker backend");
            Some(Box::new(DockerBackend::new(docker_client)))
        }
        "null" => {
            tracing::info!("Using Null backend");
            Some(Box::new(NullBackend))
        }
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let backend = initialize_backend().await.expect(
        "Failed to initialize backend. Set APP_CONTROLLER_BACKEND to 'kubernetes' or 'null'",
    );
    let api_service = OpenApiService::new(Api::new(backend), "App Controller", "0.1")
        .server("http://localhost:3000/api");

    let json_schema_endpoint = api_service.spec_endpoint();
    let yaml_schema_endpoint = api_service.spec_endpoint_yaml();
    let ui = api_service.rapidoc();

    let app = Route::new()
        .nest("/api", api_service)
        .nest("/openapi.json", json_schema_endpoint)
        .nest("/openapi.yaml", yaml_schema_endpoint)
        .nest("/doc", ui)
        .at("/", get(index));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
