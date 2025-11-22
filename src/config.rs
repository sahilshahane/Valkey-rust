use std::{fs};

use serde::Deserialize;



#[derive(Deserialize, Clone)]
pub struct Config {
    pub logs_dir: String,
    pub port: u32,
    pub wal_pool_size: usize
}



pub fn get_default_config() -> Config {
    Config { 
        logs_dir: "./logs".to_string(), 
        port: 4000,
        wal_pool_size: 2
    }
}