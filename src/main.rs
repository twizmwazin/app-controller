use app_controller::{api::Api, backend::KubernetesBackend};
use kube::Client;
use poem::{Route, Server, get, handler, listener::TcpListener, web::Redirect};
use poem_openapi::OpenApiService;

#[handler]
fn index() -> Redirect {
    Redirect::temporary("/doc")
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let k8s_client = Client::try_default().await.unwrap();
    let backend = KubernetesBackend::new(k8s_client);
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
