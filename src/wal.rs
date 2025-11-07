use std::{ path::Path, sync::Arc, time::{ SystemTime, UNIX_EPOCH} };
use sqlx::types::Decimal;
use tokio::{fs::{self, OpenOptions}, io::{self, AsyncReadExt, AsyncWriteExt}, sync::{Mutex, RwLock, RwLockWriteGuard}, time::interval};

use crate::{DBPool, error::KVStoreError};


type File = tokio::fs::File;
type WALPool = Vec<Arc<RwLock<WALFile>>>;

pub struct WALFile {
    pub file: File
}

impl WALFile {
    pub async fn write_and_flush(&mut self, buf: &[u8]) -> io::Result<()> {
        self.file.write_all(buf).await?;
        self.file.flush().await
    }
}

pub struct WAL {
    db: Arc<DBPool>,
    logs_dir: String,
    pool: WALPool
}


#[derive(Debug, Clone)]
pub enum WALOperation {
    Set { timestamp: u128, key: String, value: String },
    Delete { timestamp: u128, key: String },
}

pub struct WALDecoder {
    buffer: Vec<u8>,
    offset: usize,
}

impl WALDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            offset: 0,
        }
    }

    /// Feed more data into the decoder
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode the next operation
    pub fn next_operation(&mut self) -> Option<Result<WALOperation, String>> {
        // Need at least 1 byte for operation type
        if self.offset >= self.buffer.len() {
            return None;
        }

        let start_offset = self.offset;
        let op = self.buffer[self.offset];
        self.offset += 1;

        let result = match op {
            b'S' => self.decode_set(),
            b'D' => self.decode_delete(),
            b'\n' => {
                // Skip empty lines
                return self.next_operation();
            }
            _ => {
                self.offset = start_offset; // Rewind
                return None; // Unknown operation, might need more data
            }
        };

        match result {
            Ok(operation) => Some(Ok(operation)),
            Err(e) => {
                // Rewind on error (incomplete data)
                self.offset = start_offset;
                if e == "incomplete" {
                    None // Need more data
                } else {
                    Some(Err(e)) // Real error
                }
            }
        }
    }

    fn decode_set(&mut self) -> Result<WALOperation, String> {
        // Read timestamp (16 bytes)
        if self.offset + 16 > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let timestamp = u128::from_le_bytes([
            self.buffer[self.offset], self.buffer[self.offset + 1],
            self.buffer[self.offset + 2], self.buffer[self.offset + 3],
            self.buffer[self.offset + 4], self.buffer[self.offset + 5],
            self.buffer[self.offset + 6], self.buffer[self.offset + 7],
            self.buffer[self.offset + 8], self.buffer[self.offset + 9],
            self.buffer[self.offset + 10], self.buffer[self.offset + 11],
            self.buffer[self.offset + 12], self.buffer[self.offset + 13],
            self.buffer[self.offset + 14], self.buffer[self.offset + 15],
        ]);
        self.offset += 16;

        // Read key length (4 bytes)
        if self.offset + 4 > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let key_len = u32::from_le_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
            self.buffer[self.offset + 2],
            self.buffer[self.offset + 3],
        ]) as usize;
        self.offset += 4;

        // Read key
        if self.offset + key_len > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let key = String::from_utf8_lossy(&self.buffer[self.offset..self.offset + key_len]).to_string();
        self.offset += key_len;

        // Read value length (4 bytes)
        if self.offset + 4 > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let val_len = u32::from_le_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
            self.buffer[self.offset + 2],
            self.buffer[self.offset + 3],
        ]) as usize;
        self.offset += 4;

        // Read value
        if self.offset + val_len > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let value = String::from_utf8_lossy(&self.buffer[self.offset..self.offset + val_len]).to_string();
        self.offset += val_len;

        // Skip newline if present
        if self.offset < self.buffer.len() && self.buffer[self.offset] == b'\n' {
            self.offset += 1;
        }

        Ok(WALOperation::Set { timestamp, key, value })
    }

    fn decode_delete(&mut self) -> Result<WALOperation, String> {
        // Read timestamp (16 bytes)
        if self.offset + 16 > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let timestamp = u128::from_le_bytes([
            self.buffer[self.offset], self.buffer[self.offset + 1],
            self.buffer[self.offset + 2], self.buffer[self.offset + 3],
            self.buffer[self.offset + 4], self.buffer[self.offset + 5],
            self.buffer[self.offset + 6], self.buffer[self.offset + 7],
            self.buffer[self.offset + 8], self.buffer[self.offset + 9],
            self.buffer[self.offset + 10], self.buffer[self.offset + 11],
            self.buffer[self.offset + 12], self.buffer[self.offset + 13],
            self.buffer[self.offset + 14], self.buffer[self.offset + 15],
        ]);
        self.offset += 16;

        // Read key length (4 bytes)
        if self.offset + 4 > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let key_len = u32::from_le_bytes([
            self.buffer[self.offset],
            self.buffer[self.offset + 1],
            self.buffer[self.offset + 2],
            self.buffer[self.offset + 3],
        ]) as usize;
        self.offset += 4;

        // Read key
        if self.offset + key_len > self.buffer.len() {
            return Err("incomplete".to_string());
        }
        
        let key = String::from_utf8_lossy(&self.buffer[self.offset..self.offset + key_len]).to_string();
        self.offset += key_len;

        // Skip newline if present
        if self.offset < self.buffer.len() && self.buffer[self.offset] == b'\n' {
            self.offset += 1;
        }

        Ok(WALOperation::Delete { timestamp, key })
    }

    /// Clear processed data from buffer
    pub fn compact(&mut self) {
        if self.offset > 0 {
            self.buffer.drain(0..self.offset);
            self.offset = 0;
        }
    }
}

impl WAL {

    pub async fn get_writer_file(&self) -> Result<RwLockWriteGuard<WALFile>, KVStoreError> {
        // if self.pool.is_empty() {
        //     tracing::error!("write-ahead-logger has 0 file writers in the pool");
        //     return Err(KVStoreError::PoolError);
        // }

        // let mut rng = rand::rng();
        // let n = self.pool.len();
        // let start = rng.random_range(0..n);

        // // let f = start.randm_range(0..n);
        // // Try to find a writer we can acquire immediately
        // for i in 0..n {
        //     let idx = (start + i) % n;
        //     let writer = self.pool[idx].try_write();

        //     if let Ok(guard) = writer {
        //         return Ok(guard);
        //     }
        // }

        for i in &self.pool {
            let writer = i.try_write();

            if let Ok(guard) = writer {
                return Ok(guard);
            }
        }

       let writer = self.pool[0].write().await;
       return Ok(writer);
    }

    pub async fn set(&self, key: &str, val: &str) -> io::Result<()>{

        let time_ns = SystemTime::now().duration_since(UNIX_EPOCH);

        let key_bytes = key.as_bytes();
        let val_bytes = val.as_bytes();
        
        // Pre-calculate total size: 1 (op) + 4 (key_len) + key + 4 (val_len) + val
        let total_size = 1 + 4 + key_bytes.len() + 4 + val_bytes.len()+ 1 + 16;
        let mut buffer = Vec::with_capacity(total_size);
        
        // Build the entire buffer
        buffer.push(b'S'); // Operation type
        buffer.extend_from_slice(&time_ns.unwrap().as_nanos().to_le_bytes());
        buffer.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(key_bytes);
        buffer.extend_from_slice(&(val_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(val_bytes);
        buffer.push(b'\n');


        let mut guard = self.get_writer_file().await.map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{e:?}")))?;
        guard.write_and_flush(&buffer).await?;
        drop(guard);

        return Ok(())
    }

    pub async fn delete(&self, key: &str) -> io::Result<()>{
        let time_ns = SystemTime::now().duration_since(UNIX_EPOCH);

        let key_bytes = key.as_bytes();
        
        // Pre-calculate total size: 1 (op) + 4 (key_len) + key + 4 (val_len) + val
        let total_size = 1 + 4 + key_bytes.len() + 1 + 16;
        let mut buffer = Vec::with_capacity(total_size);
        
        // Build the entire buffer
        buffer.push(b'D'); // Operation type
        buffer.extend_from_slice(&time_ns.unwrap().as_nanos().to_le_bytes());
        buffer.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(key_bytes);
        buffer.push(b'\n');

        let mut guard = self.get_writer_file().await.map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{e:?}")))?;
        guard.write_and_flush(&buffer).await?;
        drop(guard);
        
        return Ok(())
    }

    pub async fn recover(&self) ->  std::io::Result<()> {
        // flush existing wal files to db

        let mut entries = fs::read_dir(&self.logs_dir).await?;

        let mut wals = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if !path.is_file(){
                continue;
            }

            if let Some(path) = path.to_str(){
                wals.push(path.to_string());
            }
        }

        wals.sort_unstable_by(|a, b| b.cmp(a));


        // Process WAL files in order
        for wal_file in wals {
            
            tracing::info!("Processing WAL file: {}", wal_file);
            
            let mut read_file = OpenOptions::new()
                .read(true)
                .open(&wal_file)
                .await
                .expect(&format!("Failed to read WAL log : {}", &wal_file));

            let mut decoder = WALDecoder::new();
            const CHUNK_SIZE: usize = 8192;
            let mut buffer = vec![0u8; CHUNK_SIZE];

            let mut tx = self.db.begin().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
            const BATCH_SIZE: usize = 20_000;
            let mut set_batch: Vec<(String, u128, String)> = Vec::with_capacity(BATCH_SIZE);
            let mut delete_batch: Vec<(String, u128)> = Vec::with_capacity(BATCH_SIZE);
       
            loop {
                let bytes_read = read_file.read(&mut buffer).await?;
                if bytes_read == 0 {
                    break;
                }

                // Feed data to decoder
                decoder.feed(&buffer[..bytes_read]);

                // Process all complete operations
                while let Some(result) = decoder.next_operation() {
                    match result {
                        Ok(operation) => {
                            match operation {
                                WALOperation::Set { timestamp, key, value } => {
                                    set_batch.push((key, timestamp, value));
                                }
                                WALOperation::Delete { timestamp, key } => {
                                    delete_batch.push((key, timestamp));
                                }
                            }

                            if set_batch.len() >= BATCH_SIZE || delete_batch.len() >= BATCH_SIZE {
                                self.execute_set_batch(&mut tx, &mut set_batch).await?;
                                self.execute_delete_batch(&mut tx, &mut delete_batch).await?;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to decode operation: {}", e);
                        }
                    }
                }

                // Clean up processed data
                decoder.compact();
            }

            // Execute remaining batches
            if !set_batch.is_empty() {
                self.execute_set_batch(&mut tx, &mut set_batch).await?;
            }
            if !delete_batch.is_empty() {
                self.execute_delete_batch(&mut tx, &mut delete_batch).await?;
            }

            tx.commit().await.map_err(|err| {
                tracing::error!("Failed to commit WAL file to db {wal_file} {err}");
                io::Error::new(io::ErrorKind::Other, err)
            })?;
            

            fs::remove_file(&wal_file).await.map_err(|err|{
                tracing::error!("Failed to remove WAL file {wal_file} {err}");
                io::Error::new(io::ErrorKind::Other, err)
            })?;
        }

        tracing::info!("Flushing write-ahead-logs to kv_store table");

        self.sync_db_tables().await.map_err(|err| {
            tracing::error!("Failed to sync changes from wal_sync table to kv_store table");
            io::Error::new(io::ErrorKind::Other, err)
        })?;

        tracing::info!("Successfully flushed write-ahead-logs to kv_store table");

        Ok(())
    }

    pub async fn sync_db_tables(&self) -> io::Result<()> {
        let mut tx = self.db.begin().await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Update kv_store with SET operations from wal_sync
        sqlx::query(
            "INSERT INTO kv_store (key, value) 
         SELECT key, value FROM wal_sync WHERE operation = 'SET'
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value"
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Delete kv_store entries marked for deletion in wal_sync
        sqlx::query(
            "DELETE FROM kv_store
            WHERE key IN (SELECT key FROM wal_sync WHERE operation = 'DELETE')"
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Clean up wal_sync table
        sqlx::query("DELETE FROM wal_sync")
            .execute(&mut *tx)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        tx.commit().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }

    async fn execute_set_batch(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, batch: &mut Vec<(String, u128, String)>) -> std::io::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let mut query = String::from(
            "INSERT INTO wal_sync (key, time, value, operation) VALUES "
        );
        
        for (i, _) in batch.iter().enumerate() {
            if i > 0 { query.push(','); }
            query.push_str(&format!("(${}, ${}, ${}, 'SET')", i*3+1, i*3+2, i*3+3));
        }
        
        query.push_str(
            " ON CONFLICT (key) DO UPDATE SET time = EXCLUDED.time, value = EXCLUDED.value, operation = EXCLUDED.operation \
            WHERE wal_sync.time < EXCLUDED.time"
        );

        let mut q = sqlx::query(&query);
        for (key, timestamp, value) in batch.iter() {
            q = q.bind(key).bind(Decimal::from(*timestamp)).bind(value);
        }
        
        tracing::warn_span!("sqlx", target = "sqlx::query").in_scope(|| {
            Box::pin(q.execute(&mut **tx))
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        batch.clear();
        Ok(())
    }

    async fn execute_delete_batch(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, batch: &mut Vec<(String, u128)>) -> std::io::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let mut query = String::from(
            "INSERT INTO wal_sync (key, time, operation) VALUES "
        );
        
        for (i, _) in batch.iter().enumerate() {
            if i > 0 { query.push(','); }
            query.push_str(&format!("(${}, ${}, 'DELETE')", i*2+1, i*2+2));
        }
        
        query.push_str(
            " ON CONFLICT (key) DO UPDATE SET time = EXCLUDED.time, operation = EXCLUDED.operation \
            WHERE wal_sync.time < EXCLUDED.time"
        );

        let mut q = sqlx::query(&query);
        for (key, timestamp) in batch.iter() {
            q = q.bind(key).bind(Decimal::from(*timestamp));
        }
        
        // q.execute(&mut **tx)
        //     .await
        //     .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        tracing::warn_span!("sqlx", target = "sqlx::query").in_scope(|| {
            Box::pin(q.execute(&mut **tx))
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;



        batch.clear();
        Ok(())
    }

    pub async fn sync(&self) -> io::Result<()> {

        // before syncing

        // // HOLD THE LOCK for the entire operation
        // let mut file_guard = self.file.lock().await; 

        // let mut file_path = self.file_path.lock().await;

        // // Check if WAL file is empty
        // let metadata = fs::metadata(&(*file_path)).await?;
        // if metadata.len() == 0 {
        //     tracing::info!("WAL file is empty, skipping sync");
        //     return Ok(());
        // }

        // let archive_name = format!("{}.archive", &(*file_path));
        // fs::rename(&(*file_path), &archive_name).await?;
        
        // tracing::info!("WAL archived to: {}", archive_name);

        // let new_file_path = WAL::get_new_file_name();

        // // Create new file and replace the handle in the mutex
        // let new_file = WAL::get_file(&new_file_path).await;
        // let mut read_file = std::mem::replace(&mut *file_guard, new_file);

        // *file_path = new_file_path;

        // drop(file_path);
        // drop(file_guard);

        // // Open a separate read handle

        // const CHUNK_SIZE: usize = 1024 * 8;
        // let mut buffer = vec![0u8; CHUNK_SIZE];
        // let mut leftover = Vec::new();
        
        // let mut tx = self.db.begin().await
        //     .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        // loop {
        //     let bytes_read = read_file.read(&mut buffer).await?;

        //     if bytes_read == 0 {
        //         break;
        //     }

        //     let mut data = leftover.clone();
        //     data.extend_from_slice(&buffer[..bytes_read]);
        
        //     let mut offset = 0;

        //     while offset < data.len() {
        //         if offset >= data.len() {
        //             break;
        //         }
                
        //         let op = data[offset];
        //         offset += 1;
                
        //         match op {
        //             b'S' => {
        //                 // Read timestamp (16 bytes for u128 nanoseconds)
        //                 if offset + 16 > data.len() {
        //                     offset -= 1;
        //                     break;
        //                 }
                        
        //                 let _timestamp = u128::from_le_bytes([
        //                     data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        //                     data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
        //                     data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
        //                     data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
        //                 ]);
        //                 offset += 16;
                        
        //                 if offset + 4 > data.len() {
        //                     offset -= 1 + 16;
        //                     break;
        //                 }
                        
        //                 let key_len = u32::from_le_bytes([
        //                     data[offset],
        //                     data[offset + 1],
        //                     data[offset + 2],
        //                     data[offset + 3],
        //                 ]) as usize;
        //                 offset += 4;
                        
        //                 if offset + key_len > data.len() {
        //                     offset -= 5 + 16;
        //                     break;
        //                 }
                        
        //                 let key = String::from_utf8_lossy(&data[offset..offset + key_len]).to_string();
        //                 offset += key_len;
                        
        //                 if offset + 4 > data.len() {
        //                     offset -= 5 + key_len + 16;
        //                     break;
        //                 }
                        
        //                 let val_len = u32::from_le_bytes([
        //                     data[offset],
        //                     data[offset + 1],
        //                     data[offset + 2],
        //                     data[offset + 3],
        //                 ]) as usize;
        //                 offset += 4;
                        
        //                 if offset + val_len > data.len() {
        //                     offset -= 9 + key_len + 16;
        //                     break;
        //                 }
                        
        //                 let value = String::from_utf8_lossy(&data[offset..offset + val_len]).to_string();
        //                 offset += val_len;
                        
        //                 if offset < data.len() && data[offset] == b'\n' {
        //                     offset += 1;
        //                 }
                        
        //                 sqlx::query("INSERT INTO kv_store (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        //                     .bind(&key)
        //                     .bind(&value)
        //                     .execute(&mut *tx)
        //                     .await
        //                     .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        //             }
        //             b'D' => {
        //                 // Read timestamp (16 bytes for u128 nanoseconds)
        //                 if offset + 16 > data.len() {
        //                     offset -= 1;
        //                     break;
        //                 }
                        
        //                 let _timestamp = u128::from_le_bytes([
        //                     data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        //                     data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
        //                     data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
        //                     data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
        //                 ]);
        //                 offset += 16;
                        
        //                 if offset + 4 > data.len() {
        //                     offset -= 1 + 16;
        //                     break;
        //                 }
                        
        //                 let key_len = u32::from_le_bytes([
        //                     data[offset],
        //                     data[offset + 1],
        //                     data[offset + 2],
        //                     data[offset + 3],
        //                 ]) as usize;
        //                 offset += 4;
                        
        //                 if offset + key_len > data.len() {
        //                     offset -= 5 + 16;
        //                     break;
        //                 }
                        
        //                 let key = String::from_utf8_lossy(&data[offset..offset + key_len]).to_string();
        //                 offset += key_len;
                        
        //                 if offset < data.len() && data[offset] == b'\n' {
        //                     offset += 1;
        //                 }
                        
        //                 sqlx::query("DELETE FROM kv_store WHERE key = $1")
        //                     .bind(&key)
        //                     .execute(&mut *tx)
        //                     .await
        //                     .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        //             }
        //         }
        //     }
            
        //     leftover = data[offset..].to_vec();
        // }
        
        // // Commit transaction
        // tx.commit().await
        //     .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // // Close read file
        // drop(read_file);

        Ok(())
    }

    fn get_new_file_name() -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f");
        format!("wal_{timestamp}.log")
    }

    pub async fn initialize_pool(&mut self, pool_size: usize) { 
        let mut pool = Vec::with_capacity(pool_size);


        for _ in 0..pool_size {
            pool.push(
                Arc::new(RwLock::new(
                    WALFile {
                        file: WAL::get_file(&self.logs_dir).await
                    }
                )));
        }
        
        self.pool = pool;
    }

    async fn get_file(logs_dir: &str) -> File {
        let file_name: String = WAL::get_new_file_name();
        fs::create_dir_all(logs_dir).await.unwrap();
        let file_path = Path::new(logs_dir).join(&file_name).to_string_lossy().to_string();


        OpenOptions::new()
                .append(true)
                .create(true) // Create the file if it doesn't exist
                .open(file_path)
                .await
                .expect("Failed to create WAL log file")
    }


    pub async fn new(db: Arc<DBPool>, logs_dir: &str) -> anyhow::Result<Self> {
        Ok(
            WAL { 
                db,
                logs_dir: logs_dir.to_string(),
                pool: vec![],
            }
        )
    }

    pub fn start_background_sync(self: Arc<Self>) {
        // tokio::spawn(async move {
        //     let mut interval = interval(Duration::from_secs(10));
            
        //     loop {
        //         interval.tick().await;

        //         let file_path = self.file_path.lock().await.clone();
                    
        //         // Check WAL file size
        //         match fs::metadata(&file_path).await {
        //             Ok(metadata) => {
        //                 let file_size = metadata.len();
        //                 const THRESHOLD_BYTES: u64 = 1024 * 1024 * 50;
                        
        //                 if file_size > THRESHOLD_BYTES {
        //                     tracing::info!("WAL file size: {} bytes (> {THRESHOLD_BYTES}), starting sync", file_size);
                            
        //                     match self.sync().await {
        //                         Ok(_) => {
        //                             tracing::info!("Background WAL sync completed successfully");
        //                         }
        //                         Err(e) => {
        //                             tracing::error!("Background WAL sync failed: {}", e);
        //                         }
        //                     }
        //                 } else {
        //                     tracing::debug!("WAL file size: {} bytes (< {THRESHOLD_BYTES}), skipping sync", file_size);
        //                 }
        //             }
        //             Err(e) => {
        //                 tracing::warn!("Failed to check WAL file size: {}", e);
        //             }
        //         }
        //     }
        // });
    }
}