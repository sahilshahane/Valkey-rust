use axum::{http::StatusCode, response::IntoResponse};
use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::FmtSubscriber;
use tower_http::trace::TraceLayer;
use std::env;
use std::sync::Arc;
use whirlwind::ShardMap;


use crate::db_connection::load_kvstore_inmemory;
use crate::wal::WAL;

mod handlers;
mod db_connection;
mod models;
mod error;
mod wal;
mod constants;


pub type DBPool = sqlx::SqlitePool;
pub type HashMap = ShardMap<String, String>;

#[derive(Clone)]
pub struct AppState{
    pool: Arc<DBPool>,
    cache: Arc<HashMap>,
    wal: Arc<WAL>
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}



#[tokio::main]
async fn main() {
    // Load .env file at the start of your application
    dotenvy::dotenv().ok();

    let is_dev = match env::var("ENV") {
        Ok(val) => val == "development",
        Err(_) => true,
    };

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        // Only show INFO and ERROR messages (skips DEBUG and TRACE)
        .with_max_level(if is_dev{ LevelFilter::INFO } else { LevelFilter::ERROR })
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
    
    let pool = Arc::new(pool);
    let cache = Arc::new(HashMap::new());

    let wal = Arc::new(WAL::new(pool.clone()).await);
      
    tracing::info!("syncing write-ahead-log");

    if let Err(error) = wal.sync().await {
        tracing::error!("Failed to sync write-ahead-log & database\n{error}");
        return;
    }
    
    tracing::info!("syncing of write-ahead-log with database completed");

    if let Err(error) = load_kvstore_inmemory(&cache, &pool).await {
        tracing::error!("Failed to load database data in in-memory data structure\n{error}");
        return;
    };

    // Start background sync task
    wal.clone().start_background_sync();
    tracing::info!("Started background WAL sync task");


  
    let state = Arc::new(AppState { pool, cache, wal });

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/key/{key}", 
        get(handlers::get_key)
        .post(handlers::set_key)
        .delete(handlers::delete_key))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

     // Start server
    let addr = "0.0.0.0:4000";
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}