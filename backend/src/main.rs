use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod calculation;
mod pdf;
mod routes;
mod storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "clrdbt=debug,tower_http=debug".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/api/generate", post(routes::generate::handle))
        .route("/api/deliver", get(routes::deliver::handle))
        .route("/api/webhook/stripe", post(routes::webhook::handle))
        .route("/health", get(|| async { "ok" }))
        .nest_service("/", ServeDir::new("../frontend"))
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:8080";
    tracing::info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
