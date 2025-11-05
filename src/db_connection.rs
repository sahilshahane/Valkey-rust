use core::panic;
use std::env;
use std::fs;
use std::path::Path;


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