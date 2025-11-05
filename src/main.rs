use axum::{http::StatusCode, response::IntoResponse};
use axum::{routing::get, Router};
use dashmap::DashMap;
use tokio::net::TcpListener;
use tracing::Level;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::FmtSubscriber;
use tower_http::trace::TraceLayer;
use std::env;
use std::sync::Arc;

mod handlers;
mod db_connection;
mod models;
mod error;


#[derive(Clone)]
pub struct AppState {
    pool: sqlx::SqlitePool,
    cache: Arc<DashMap<String, String>>
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}



#[tokio::main]
async fn main() {
    // Load .env file at the start of your application
    dotenvy::dotenv().ok();

    let isDev = match env::var("ENV") {
        Ok(val) => val == "development",
        Err(_) => true,
    };

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(if isDev{ LevelFilter::TRACE } else { LevelFilter::OFF})
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Database connection
    let pool = db_connection::get_sqlite_connection();

    // Run migrations
    if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
        tracing::error!("Failed to run migrations: {}", err);
        panic!("Database migration failed");
    }

    tracing::info!("Database migrations completed successfully");

    let cache = Arc::new(DashMap::new());

      // Shared application state
    let state = Arc::new(AppState { pool, cache });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/key/{key}", 
        get(handlers::get_key)
        .post(handlers::set_key)
        .delete(handlers::delete_key))
        // .route("/api/kv", get(handlers::list_all))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

     // Start server
    let addr = "0.0.0.0:4000";
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}