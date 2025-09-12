use poem::{listener::TcpListener, Route, EndpointExt, middleware::AddData};
use poem_openapi::OpenApiService;
use sea_orm::DbConn;

mod api;
mod db;


use api::{UserApi, GeminiApi,MemoApi,Api};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Initialize tracing (optional, but good for debugging)
    tracing_subscriber::fmt::init();

    // Connect to DB
    let db: DbConn = db::connect().await.expect("Database connection failed");

    // OpenAPI service (combined APIs)
    let api_service = OpenApiService::new((UserApi, GeminiApi, MemoApi,Api), "Smart Memo API", "1.0")
        .server("http://localhost:4000/api");

    // Create the Swagger UI endpoint
    let ui = api_service.swagger_ui();

    // Build application
    let app = Route::new()
        // Pass the database connection into the API handlers
        // This makes `DbConn` available to any handler using `poem_openapi::param::Data<DbConn>`
        .nest("/api", api_service.with(AddData::new(db))) 
        .nest("/", ui);

    // Start server
    poem::Server::new(TcpListener::bind("0.0.0.0:4000"))
        .run(app)
        .await
}