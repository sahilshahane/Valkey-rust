use core::panic;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::task::futures;

use crate::DBPool;
use crate::HashMap;


fn get_sqlite_db_url() -> Option<String> {
    let database_url= &env::var("SQLITE_DB").expect("Failed to load SQLITE_DB env variable");
    
    let parent_path: &Path = Path::new(database_url).parent()?;

    if !parent_path.exists() {
        fs::create_dir_all(parent_path).unwrap();
    }

    if !Path::new(database_url).exists() {
        println!("Creating SQLite file...");
        fs::File::create(database_url).expect("Failed to create file");
    }

    return Some(database_url.clone());
}

    
pub fn get_sqlite_connection() -> sqlx::SqlitePool {
    let database_url = &get_sqlite_db_url().unwrap();
  
    let pool = match sqlx::SqlitePool::connect_lazy(database_url) {
        Ok(pool) => pool,
        Err(err) => panic!("{err}"),
    };

    pool
}

pub fn get_pg_connection() -> PgPool {
    let database_url = &env::var("PG_DB").unwrap();

    let pool = match PgPoolOptions::new()
    .max_connections(10)
    .connect_lazy(database_url) {
        Ok(pool) => pool,
        Err(err) => panic!("{err}"),
    };

    pool
}


pub async fn load_kvstore_inmemory(map: &Arc<HashMap>, pool: &DBPool) -> Result<(), sqlx::Error> {

    tracing::info!("Loading key-value data to memory");

    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM kv_store"
    )
    .fetch_all(pool)
    .await?;


    tracing::info!("Data fetched from the database");
    
    tracing::info!("Inserting data to in-memory data structure");
   
    let chunk_size = 5000;
    let mut handles = Vec::new();

    for chunk in rows.chunks(chunk_size) {

        for (key, value) in chunk {

            let map_clone = map.clone();
            let key_clone = key.clone();
            let value_clone = value.clone();
            
            let handle = tokio::spawn(async move {
                map_clone.insert(key_clone, value_clone);
            });
            
            handles.push(handle);
        } 
    }

    for handle in handles {
        handle.await.unwrap();
    }

    tracing::info!("In-memory data loading completed");

    Ok(())
}