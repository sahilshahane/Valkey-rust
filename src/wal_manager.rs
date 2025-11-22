use std::{ path::Path, sync::Arc, time::{ SystemTime, UNIX_EPOCH, Duration} };
use sqlx::types::Decimal;
use tokio::{fs::{self, OpenOptions}, io::{self, AsyncReadExt, AsyncWriteExt}, sync::{Mutex, RwLock, RwLockWriteGuard, mpsc}, time::{interval, sleep, timeout}};

use crate::{DBPool, error::KVStoreError};


type File = tokio::fs::File;
type WALPool = Vec<Arc<RwLock<WALFile>>>;

pub struct WALFile {
    pub file: File
}

impl WALFile {
    pub async fn write_and_flush(&mut self, buf: &[u8]) -> io::Result<()> {
        self.file.write_all(buf).await?;
        self.file.sync_data().await
    }
}

pub struct WAL {
    db: Arc<DBPool>,
    logs_dir: String,
    pool: WALPool,
    pool_size: usize,
    tx: mpsc::UnboundedSender<Vec<u8>>,
    rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>>,
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

    async fn try_acquire_walfile_lock(lock_file: &str) -> io::Result<Option<tokio::fs::File>> {
        const LOCK_STALE_TIMEOUT_SECS: u64 = 600; // 10 minutes - consider lock stale if not updated

        // Check if lock file exists and get its age
        if let Ok(metadata) = fs::metadata(lock_file).await {
            if let Ok(elapsed) = metadata.modified()?.elapsed() {
                if elapsed.as_secs() > LOCK_STALE_TIMEOUT_SECS {
                    tracing::warn!("Lock file is stale (age: {:?}), removing and retrying...", elapsed);
                    fs::remove_file(lock_file).await.ok();
                } else {
                    // Lock is fresh, another process is likely still processing
                    tracing::warn!("Cannot acquire lock for file: {}, skipping...", lock_file);
                    return Ok(None);
                }
            }
        }

        // Try to create lock file
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_file)
            .await
        {
            Ok(file) => Ok(Some(file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                tracing::warn!("Cannot acquire lock for file: {} (already exists), skipping...", lock_file);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    async fn touch_walfile_lock(lock_file: &str) -> io::Result<()> {
        // Update the file's modified time by opening and closing it
        OpenOptions::new()
            .write(true)
            .open(lock_file)
            .await?
            .sync_all()
            .await?;
        Ok(())
    }

    pub async fn get_writer_file(&self) -> Result<RwLockWriteGuard<WALFile>, KVStoreError> {
        for i in &self.pool {
            let writer = i.try_write();

            if let Ok(guard) = writer {
                return Ok(guard);
            }
        }

       let writer = self.pool[fastrand::usize(0..self.pool_size)].write().await;
       return Ok(writer);
    }

    pub fn set(&self, key: &str, val: &str) -> io::Result<()>{

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

        // Send to background writer channel
        self.tx.send(buffer)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to send to WAL channel: {}", e)))?;

        return Ok(())
    }

    pub fn delete(&self, key: &str) -> io::Result<()>{
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

        // Send to background writer channel
        self.tx.send(buffer)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to send to WAL channel: {}", e)))?;
        
        return Ok(())
    }
    
    pub async fn recover_file(&self, wal_file: &str) ->  std::io::Result<()> {
        tracing::info!("Processing WAL file: {}", wal_file);
        
        // Create lock file path
        let lock_file = format!("{}.lock", wal_file);
        
        // Try to acquire lock
        let _lock_file_handle = match Self::try_acquire_walfile_lock(&lock_file).await {
            Ok(Some(handle)) => handle,
            Ok(None) => {
                // Could not acquire lock, return error
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Cannot acquire lock for WAL file: {}", wal_file)
                ));
            }
            Err(e) => {
                tracing::error!("Error trying to acquire lock for {}: {}", wal_file, e);
                return Err(e);
            }
        };
        
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
        
        // Heartbeat interval for lock file
        let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
   
        loop {
            tokio::select! {
                bytes_result = read_file.read(&mut buffer) => {
                    let bytes_read = bytes_result?;
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
                _ = heartbeat_interval.tick() => {
                    // Update lock file timestamp
                    if let Err(e) = Self::touch_walfile_lock(&lock_file).await {
                        tracing::warn!("Failed to update lock file heartbeat: {}", e);
                    } else {
                        tracing::debug!("Updated lock file heartbeat for: {}", wal_file);
                    }
                }
            }
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
        
        // Remove WAL file
        fs::remove_file(&wal_file).await.map_err(|err|{
            tracing::error!("Failed to remove WAL file {wal_file} {err}");
            io::Error::new(io::ErrorKind::Other, err)
        })?;
        
        // Remove lock file
        fs::remove_file(&lock_file).await.map_err(|err|{
            tracing::error!("Failed to remove lock file {lock_file} {err}");
            io::Error::new(io::ErrorKind::Other, err)
        })?;
        
        Ok(())
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
            if let Err(e) = self.recover_file(&wal_file).await {
                tracing::error!("Failed to recover WAL file {}: {}", wal_file, e);
                // Continue to next file even if one fails
            }
        }

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
        
        self.pool_size = pool.len();
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
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        
        Ok(WAL { 
            db,
            logs_dir: logs_dir.to_string(),
            pool: vec![],
            pool_size: 0,
            tx,
            rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    pub async fn start_background_writer(self: Arc<Self>) {
        // Take ownership of the receiver from the WAL struct
        let rx = {
            let mut rx_option = self.rx.lock().await;
            rx_option.take()
        };
        
        if let Some(rx) = rx {
            tokio::spawn({
                let wal = Arc::clone(&self);
                async move {
                    wal.background_writer_impl(rx).await;
                }
            });
        } else {
            tracing::error!("Background writer already started or receiver already taken");
        }
    }

    /// Background writer task that receives buffered data from the channel
    /// and writes it to the WAL file on a separate async task
    /// Flushes when buffer reaches 64KB OR 10ms has passed since last flush
    async fn background_writer_impl(&self, mut rx: mpsc::UnboundedReceiver<Vec<u8>>) {
        tracing::info!("WAL background writer started");
        
        const FLUSH_THRESHOLD: usize = 64 * 1024; // 64KB threshold for flushing
        let mut buffer = Vec::with_capacity(FLUSH_THRESHOLD);
        
        const FLUSH_TIMEOUT_MS: u64 = 10; // 10ms timeout

        let mut flush_timer = tokio::time::interval(Duration::from_millis(FLUSH_TIMEOUT_MS));
        flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Receive new data from channel
                data_opt = rx.recv() => {
                    match data_opt {
                        Some(data) => {
                            buffer.extend_from_slice(&data);

                            // If buffer exceeds threshold, flush immediately
                            if buffer.len() >= FLUSH_THRESHOLD {
                                if let Err(e) = self.flush_buffer(&buffer).await {
                                    tracing::error!("Failed to flush WAL buffer: {}", e);
                                }
                                buffer.clear();
                                flush_timer.reset(); // Reset timer after flush
                            }
                        }
                        None => {
                            // Channel closed, flush remaining data and exit
                            if !buffer.is_empty() {
                                if let Err(e) = self.flush_buffer(&buffer).await {
                                    tracing::error!("Failed to flush remaining WAL buffer: {}", e);
                                }
                            }
                            tracing::info!("WAL background writer shutting down");
                            return;
                        }
                    }
                }
                
                // Timer tick - flush buffer if it has data
                _ = flush_timer.tick() => {
                    if !buffer.is_empty() {
                        if let Err(e) = self.flush_buffer(&buffer).await {
                            tracing::error!("Failed to flush WAL buffer on timeout: {}", e);
                        }
                        buffer.clear();
                    }
                }
            }
        }
    }

    /// Flush accumulated buffer to WAL file
    async fn flush_buffer(&self, buffer: &[u8]) -> io::Result<()> {
        let mut guard = self.get_writer_file().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{e:?}")))?;
        guard.write_and_flush(buffer).await?;
        Ok(())
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