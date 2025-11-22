#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;


use axum::{http::StatusCode, response::IntoResponse};
use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use tower_http::trace::TraceLayer;
use std::env;
use std::sync::Arc;
use whirlwind::ShardMap;
use dashmap::DashMap;

use crate::config::{Config, get_default_config};
use crate::db_connection::load_kvstore_inmemory;
use crate::wal_manager::WAL;

mod handlers;
mod db_connection;
mod models;
mod error;
mod wal_manager;
mod constants;
mod config;


pub type DBPool = sqlx::PgPool;
// pub type HashMap = ShardMap<String, String>;
pub type HashMap = DashMap<String, String>;

#[derive(Clone)]
pub struct AppState{
    pool: Arc<DBPool>,
    cache: Arc<HashMap>,
    wal: Arc<WAL>,
    config: Config
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}


// #[tokio::main(flavor = "current_thread")]
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // Load .env file at the start of your application
    dotenvy::dotenv().ok();

    let config = get_default_config();

    let is_dev = match env::var("ENV") {
        Ok(val) => val == "development",
        Err(_) => true,
    };
    
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        // Only show INFO and ERROR messages (skips DEBUG and TRACE)
        .with_max_level(if is_dev{ LevelFilter::INFO } else { LevelFilter::ERROR })
        .with_env_filter(EnvFilter::new("info,sqlx::query=error"))
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");


     #[cfg(not(target_env = "msvc"))]
    tracing::info!("✅ Using jemalloc allocator for better performance");
    
    #[cfg(target_env = "msvc")]
    tracing::warn!("⚠️  Using system allocator (jemalloc not available on MSVC)");

    // Database connection
    let pool = db_connection::get_pg_connection();
    
    // Run migrations
    if let Err(err) = sqlx::migrate!("./migrations").run(&pool).await {
        tracing::error!("Failed to run migrations: {}", err);
        panic!("Database migration failed");
    }
    
    tracing::info!("Database migrations completed successfully");
    
    let pool = Arc::new(pool);
    let cache = Arc::new(HashMap::new());

    // tracing::error!("Failed to create directory for write-ahead-logs, path={logs_dir}\n{err}");

    // Initialize WAL and handle potential initialization error so `wal` is Arc<WAL>
    let mut wal = match WAL::new(pool.clone(), &config.logs_dir).await {
        Ok(w) => w,
        Err(error) => {
            tracing::error!("Failed to initialize write-ahead-log {error}");
            return;
        }
    };


    if let Err(err) = wal.recover().await {
        tracing::error!("Failed to recover from WAL logs. {err}");
        return;
    }


    tracing::info!("Flushing write-ahead-logs to kv_store table");

    if let Err(err) = wal.sync_db_tables().await {
        tracing::error!("Failed to sync changes from wal_sync table to kv_store table {err}");
        return;
    }

    tracing::info!("Successfully flushed write-ahead-logs to kv_store table");


    wal.initialize_pool(config.wal_pool_size).await;
      
    tracing::info!("syncing write-ahead-log");

    // if let Err(error) = wal.sync().await {
    //     tracing::error!("Failed to sync write-ahead-log & database\n{error}");
    //     return;
    // }
    
    tracing::info!("syncing of write-ahead-log with database completed");

    if let Err(error) = load_kvstore_inmemory(&cache, &pool).await {
        tracing::error!("Failed to load database data in in-memory data structure\n{error}");
        return;
    };

    // Wrap WAL in Arc for background task
    let wal = Arc::new(wal);
    
    // Start background writer task on a separate core
    wal.clone().start_background_writer().await;
    tracing::info!("Started background WAL writer task");

    let state = Arc::new(AppState { pool, cache, wal, config: config.clone() });

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
    let addr = &format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
