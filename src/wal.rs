use std::{ sync::Arc, time::Duration };

use tokio::{fs::{self, OpenOptions}, io::{self, AsyncReadExt, AsyncWriteExt}, sync::Mutex, time::interval};

use crate::DBPool;


type File = tokio::fs::File;

pub struct WAL {
    file_path: String,
    file: Mutex<tokio::fs::File>,
    db: Arc<DBPool>
}

impl WAL {
    pub async fn set(&self, key: &str, val: &str) -> io::Result<()>{

        let key_bytes = key.as_bytes();
        let val_bytes = val.as_bytes();
        
        // Pre-calculate total size: 1 (op) + 4 (key_len) + key + 4 (val_len) + val
        let total_size = 1 + 4 + key_bytes.len() + 4 + val_bytes.len()+ 1;
        let mut buffer = Vec::with_capacity(total_size);
        
        // Build the entire buffer
        buffer.push(b'S'); // Operation type
        buffer.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(key_bytes);
        buffer.extend_from_slice(&(val_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(val_bytes);
        buffer.push(b'\n');

        let mut file_guard = self.file.lock().await;


        file_guard.write_all(&buffer).await?;

        // Flush the buffer to ensure data is written to the underlying system
        file_guard.flush().await?;

        return Ok(())
    }

    pub async fn delete(&self, key: &str) -> io::Result<()>{

        let key_bytes = key.as_bytes();
        
        // Pre-calculate total size: 1 (op) + 4 (key_len) + key + 4 (val_len) + val
        let total_size = 1 + 4 + key_bytes.len() + 1;
        let mut buffer = Vec::with_capacity(total_size);
        
        // Build the entire buffer
        buffer.push(b'D'); // Operation type
        buffer.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(key_bytes);
        buffer.push(b'\n');

        let mut file_guard = self.file.lock().await;

        file_guard.write_all(&buffer).await?;

        // Flush the buffer to ensure data is written to the underlying system
        file_guard.flush().await?;

        return Ok(())
    }

    // on evict -> sync_db

    pub async fn commit(&self) -> io::Result<()>{
        // Pre-calculate total size: 1 (op)
        let total_size = 2;
        let mut buffer = Vec::with_capacity(total_size);
        
        // Build the entire buffer
        buffer.push(b'C'); // Operation type
        buffer.push(b'\n');

        let mut file_guard = self.file.lock().await;

        file_guard.write_all(&buffer).await?;

        // Flush the buffer to ensure data is written to the underlying system
        file_guard.flush().await?;

        return Ok(())
    }

    pub async fn sync(&self) -> io::Result<()> {
        // HOLD THE LOCK for the entire operation
        let mut file_guard = self.file.lock().await; 

        // Check if WAL file is empty
        let metadata = fs::metadata(&self.file_path).await?;
        if metadata.len() == 0 {
            tracing::info!("WAL file is empty, skipping sync");
            return Ok(());
        }

        // Open a separate read handle
        let mut read_file = OpenOptions::new()
            .read(true)
            .open(&self.file_path)
            .await?;

        const CHUNK_SIZE: usize = 1024 * 8;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut leftover = Vec::new();
        
        let mut tx = self.db.begin().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        loop {
            let bytes_read = read_file.read(&mut buffer).await?;

            if bytes_read == 0 {
                break;
            }

            let mut data = leftover.clone();
            data.extend_from_slice(&buffer[..bytes_read]);
        
            let mut offset = 0;

            while offset < data.len() {
                if offset >= data.len() {
                    break;
                }
                
                let op = data[offset];
                offset += 1;
                
                match op {
                    b'S' => {
                        if offset + 4 > data.len() {
                            offset -= 1;
                            break;
                        }
                        
                        let key_len = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]) as usize;
                        offset += 4;
                        
                        if offset + key_len > data.len() {
                            offset -= 5;
                            break;
                        }
                        
                        let key = String::from_utf8_lossy(&data[offset..offset + key_len]).to_string();
                        offset += key_len;
                        
                        if offset + 4 > data.len() {
                            offset -= 5 + key_len;
                            break;
                        }
                        
                        let val_len = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]) as usize;
                        offset += 4;
                        
                        if offset + val_len > data.len() {
                            offset -= 9 + key_len;
                            break;
                        }
                        
                        let value = String::from_utf8_lossy(&data[offset..offset + val_len]).to_string();
                        offset += val_len;
                        
                        if offset < data.len() && data[offset] == b'\n' {
                            offset += 1;
                        }
                        
                        sqlx::query("INSERT INTO kv_store (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(&key)
                            .bind(&value)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    }
                    b'D' => {
                        if offset + 4 > data.len() {
                            offset -= 1;
                            break;
                        }
                        
                        let key_len = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]) as usize;
                        offset += 4;
                        
                        if offset + key_len > data.len() {
                            offset -= 5;
                            break;
                        }
                        
                        let key = String::from_utf8_lossy(&data[offset..offset + key_len]).to_string();
                        offset += key_len;
                        
                        if offset < data.len() && data[offset] == b'\n' {
                            offset += 1;
                        }
                        
                        sqlx::query("DELETE FROM kv_store WHERE key = $1")
                            .bind(&key)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    }
                    b'C' => {
                        if offset < data.len() && data[offset] == b'\n' {
                            offset += 1;
                        }
                    }
                    b'\n' => {
                        continue;
                    }
                    _ => {
                        tracing::warn!("Unknown WAL operation: {}", op);
                        break;
                    }
                }
            }
            
            leftover = data[offset..].to_vec();
        }
        
        // Commit transaction
        tx.commit().await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Close read file
        drop(read_file);

        // Archive old file (still holding the lock!)
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let archive_name = format!("wal_{}.log.archive", timestamp);
        fs::rename("wal.log", &archive_name).await?;
        
        tracing::info!("WAL archived to: {}", archive_name);

        // Create new file and replace the handle in the mutex
        let new_file = WAL::get_file(&self.file_path).await;
        
        // Replace file handle BEFORE releasing lock
        *file_guard = new_file;

        Ok(())
    }



    async fn get_file(file_path: &str) -> File {
        OpenOptions::new()
                .append(true)
                .create(true) // Create the file if it doesn't exist
                .open(file_path)
                .await
                .expect("Failed to create WAL log file")
    }

    pub async fn new(db: Arc<DBPool>) -> Self {
        let file_path = "wal.log";
        let file = Mutex::new( WAL::get_file("wal.log").await );

        WAL { 
            file_path: file_path.to_string(), 
            file, 
            db 
        }
    }

    pub fn start_background_sync(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // Check WAL file size
                match fs::metadata(&self.file_path).await {
                    Ok(metadata) => {
                        let file_size = metadata.len();
                        const THRESHOLD_BYTES: u64 = 1024 * 1024 * 50;
                        
                        if file_size > THRESHOLD_BYTES {
                            tracing::info!("WAL file size: {} bytes (> {THRESHOLD_BYTES}), starting sync", file_size);
                            
                            match self.sync().await {
                                Ok(_) => {
                                    tracing::info!("Background WAL sync completed successfully");
                                }
                                Err(e) => {
                                    tracing::error!("Background WAL sync failed: {}", e);
                                }
                            }
                        } else {
                            tracing::debug!("WAL file size: {} bytes (< {THRESHOLD_BYTES}), skipping sync", file_size);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to check WAL file size: {}", e);
                    }
                }
            }
        });
    }
}