use app_controller::{api::Api, backend::BackendImpl};
use poem::{get, handler, listener::TcpListener, web::Redirect, Route, Server};
use poem_openapi::OpenApiService;

#[handler]
fn index() -> Redirect {
    Redirect::temporary("/doc")
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let api_service =
        OpenApiService::new(Api::from(BackendImpl::default()), "App Controller", "0.1")
            .server("http://localhost:3000/api");

    let ui = api_service.redoc();

    let app = Route::new()
        .nest("/api", api_service)
        .nest("/doc", ui)
        .at("/", get(index));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
}
