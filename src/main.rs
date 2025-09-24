use std::env;
use poem::{listener::TcpListener, Route, EndpointExt, middleware::AddData};
use poem_openapi::OpenApiService;
use sea_orm::DbConn;

mod api;
mod db;

use api::{UserApi, GeminiApi, MemoApi, Api};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Initialize tracing (optional)
    tracing_subscriber::fmt::init();

    // Connect to DB
    let db: DbConn = db::connect().await.expect("Database connection failed");

    // OpenAPI service (combined APIs)
    let api_service = OpenApiService::new((UserApi, GeminiApi, MemoApi, Api), "Smart Memo API", "1.0")
        .server("/api"); // Don't hardcode localhost here, relative path is better for deployment

    let ui = api_service.swagger_ui();

    // Build application
    let app = Route::new()
        .nest("/api", api_service.with(AddData::new(db)))
        .nest("/", ui);

    // Get PORT from environment variable (Render sets this automatically)
    let port = env::var("PORT").unwrap_or_else(|_| "4000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("🚀 Starting server on {}", addr);

    poem::Server::new(TcpListener::bind(addr))
        .run(app)
        .await
}
