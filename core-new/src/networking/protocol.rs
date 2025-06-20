//! File transfer and communication protocols

use async_stream::stream;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio_stream::Stream;

use crate::networking::{connection::NetworkConnection, Result, NetworkError};


/// File transfer header containing metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileHeader {
    /// File name
    pub name: String,
    
    /// Total size in bytes
    pub size: u64,
    
    /// Blake3 hash for verification
    pub hash: [u8; 32],
    
    /// Optional: Resume from offset
    pub resume_offset: Option<u64>,
    
    /// Chunk size for streaming
    pub chunk_size: usize,
    
    /// File metadata
    pub metadata: FileMetadata,
}

/// File metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    /// File creation time
    pub created: Option<chrono::DateTime<chrono::Utc>>,
    
    /// File modification time
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
    
    /// File permissions (Unix-style)
    pub permissions: Option<u32>,
    
    /// MIME type if known
    pub mime_type: Option<String>,
}

impl FileHeader {
    /// Create header from file path
    pub async fn from_path(path: &Path) -> Result<Self> {
        use blake3::Hasher;
        
        let file = File::open(path).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        let metadata = file.metadata().await
            .map_err(|e| NetworkError::IoError(e))?;
        
        // Calculate hash
        let mut hasher = Hasher::new();
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0u8; 8192];
        
        loop {
            let n = reader.read(&mut buffer).await
                .map_err(|e| NetworkError::IoError(e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        
        let hash = hasher.finalize();
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(hash.as_bytes());
        
        // Get file metadata
        let file_metadata = FileMetadata {
            created: metadata.created().ok().map(|t| chrono::DateTime::from(t)),
            modified: metadata.modified().ok().map(|t| chrono::DateTime::from(t)),
            permissions: None, // TODO: Extract permissions
            mime_type: None,   // TODO: Detect MIME type
        };
        
        Ok(FileHeader {
            name: path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            size: metadata.len(),
            hash: hash_bytes,
            resume_offset: None,
            chunk_size: 1024 * 1024, // 1MB chunks
            metadata: file_metadata,
        })
    }
    
    /// Verify file against header
    pub async fn verify_file(&self, path: &Path) -> Result<bool> {
        use blake3::Hasher;
        
        let file = File::open(path).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        let metadata = file.metadata().await
            .map_err(|e| NetworkError::IoError(e))?;
        
        // Check size first
        if metadata.len() != self.size {
            return Ok(false);
        }
        
        // Calculate hash
        let mut hasher = Hasher::new();
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0u8; 8192];
        
        loop {
            let n = reader.read(&mut buffer).await
                .map_err(|e| NetworkError::IoError(e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        
        let hash = hasher.finalize();
        Ok(hash.as_bytes() == &self.hash)
    }
}

/// File transfer progress information
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub transfer_rate: f64, // bytes per second
    pub eta_seconds: Option<u64>,
}

impl TransferProgress {
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            100.0
        } else {
            (self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0
        }
    }
}

/// File transfer implementation
pub struct FileTransfer;

impl FileTransfer {
    /// Send file over connection with progress tracking
    pub async fn send_file<F>(
        connection: &mut dyn NetworkConnection,
        path: &Path,
        mut progress_callback: F,
    ) -> Result<()>
    where
        F: FnMut(TransferProgress),
    {
        // Create and send header
        let header = FileHeader::from_path(path).await?;
        let header_data = serde_json::to_vec(&header)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to serialize header: {}", e)))?;
        connection.send(&header_data).await?;
        
        // Open file for reading
        let file = File::open(path).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0u8; header.chunk_size];
        let mut bytes_sent = 0u64;
        let start_time = std::time::Instant::now();
        
        // Stream file chunks
        loop {
            let n = reader.read(&mut buffer).await
                .map_err(|e| NetworkError::IoError(e))?;
            
            if n == 0 {
                break; // EOF
            }
            
            // Send chunk
            connection.send(&buffer[..n]).await?;
            bytes_sent += n as u64;
            
            // Update progress
            let elapsed = start_time.elapsed().as_secs_f64();
            let transfer_rate = if elapsed > 0.0 {
                bytes_sent as f64 / elapsed
            } else {
                0.0
            };
            
            let eta_seconds = if transfer_rate > 0.0 {
                Some(((header.size - bytes_sent) as f64 / transfer_rate) as u64)
            } else {
                None
            };
            
            let progress = TransferProgress {
                bytes_transferred: bytes_sent,
                total_bytes: header.size,
                transfer_rate,
                eta_seconds,
            };
            
            progress_callback(progress);
        }
        
        Ok(())
    }
    
    /// Receive file over connection with progress tracking
    pub async fn receive_file<F>(
        connection: &mut dyn NetworkConnection,
        output_path: &Path,
        mut progress_callback: F,
    ) -> Result<FileHeader>
    where
        F: FnMut(TransferProgress),
    {
        // Receive header
        let header_data = connection.receive().await?;
        let header: FileHeader = serde_json::from_slice(&header_data)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to deserialize header: {}", e)))?;
        
        // Create output file
        let mut file = File::create(output_path).await
            .map_err(|e| NetworkError::IoError(e))?;
        
        let mut bytes_received = 0u64;
        let start_time = std::time::Instant::now();
        
        // Receive chunks
        while bytes_received < header.size {
            let chunk_data = connection.receive().await?;
            
            file.write_all(&chunk_data).await
                .map_err(|e| NetworkError::IoError(e))?;
            
            bytes_received += chunk_data.len() as u64;
            
            // Update progress
            let elapsed = start_time.elapsed().as_secs_f64();
            let transfer_rate = if elapsed > 0.0 {
                bytes_received as f64 / elapsed
            } else {
                0.0
            };
            
            let eta_seconds = if transfer_rate > 0.0 {
                Some(((header.size - bytes_received) as f64 / transfer_rate) as u64)
            } else {
                None
            };
            
            let progress = TransferProgress {
                bytes_transferred: bytes_received,
                total_bytes: header.size,
                transfer_rate,
                eta_seconds,
            };
            
            progress_callback(progress);
        }
        
        // Flush file
        file.sync_all().await
            .map_err(|e| NetworkError::IoError(e))?;
        
        // Verify received file
        if !header.verify_file(output_path).await? {
            return Err(NetworkError::ProtocolError("File verification failed".to_string()));
        }
        
        Ok(header)
    }
    
    /// Stream file chunks as async stream
    pub fn stream_file_chunks(
        path: &Path,
        chunk_size: usize,
    ) -> impl Stream<Item = Result<Vec<u8>>> + '_ {
        stream! {
            match File::open(path).await {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let mut buffer = vec![0u8; chunk_size];
                    
                    loop {
                        match reader.read(&mut buffer).await {
                            Ok(0) => break, // EOF
                            Ok(n) => yield Ok(buffer[..n].to_vec()),
                            Err(e) => {
                                yield Err(NetworkError::IoError(e));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(NetworkError::IoError(e));
                }
            }
        }
    }
}

/// Protocol messages for communication
#[derive(Serialize, Deserialize, Debug)]
pub enum ProtocolMessage {
    /// File transfer request
    FileTransferRequest {
        header: FileHeader,
    },
    
    /// File transfer response
    FileTransferResponse {
        accepted: bool,
        reason: Option<String>,
    },
    
    /// File chunk data
    FileChunk {
        sequence: u64,
        data: Vec<u8>,
        is_final: bool,
    },
    
    /// Sync pull request
    SyncPullRequest {
        from_seq: u64,
        limit: Option<usize>,
    },
    
    /// Sync pull response
    SyncPullResponse {
        changes: Vec<SyncLogEntry>,
        has_more: bool,
        next_seq: u64,
    },
    
    /// Ping message for keepalive
    Ping {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Pong response
    Pong {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Error message
    Error {
        code: u32,
        message: String,
    },
}

/// Sync log entry for replication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncLogEntry {
    pub seq: u64,
    pub operation: String,
    pub data: Vec<u8>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub device_id: uuid::Uuid,
}

/// Protocol handler for managing communication
pub struct ProtocolHandler;

impl ProtocolHandler {
    /// Send protocol message
    pub async fn send_message(
        connection: &mut dyn NetworkConnection,
        message: ProtocolMessage,
    ) -> Result<()> {
        let data = serde_json::to_vec(&message)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to serialize message: {}", e)))?;
        connection.send(&data).await
    }
    
    /// Receive protocol message
    pub async fn receive_message(
        connection: &mut dyn NetworkConnection,
    ) -> Result<ProtocolMessage> {
        let data = connection.receive().await?;
        let message: ProtocolMessage = serde_json::from_slice(&data)
            .map_err(|e| NetworkError::SerializationError(format!("Failed to deserialize message: {}", e)))?;
        Ok(message)
    }
    
    /// Handle ping-pong keepalive
    pub async fn handle_ping(
        connection: &mut dyn NetworkConnection,
        ping: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let pong = ProtocolMessage::Pong {
            timestamp: ping,
        };
        Self::send_message(connection, pong).await
    }
    
    /// Send error message
    pub async fn send_error(
        connection: &mut dyn NetworkConnection,
        code: u32,
        message: String,
    ) -> Result<()> {
        let error_msg = ProtocolMessage::Error { code, message };
        Self::send_message(connection, error_msg).await
    }
}

/// File transfer session for managing ongoing transfers
pub struct TransferSession {
    pub transfer_id: uuid::Uuid,
    pub header: FileHeader,
    pub bytes_transferred: u64,
    pub start_time: std::time::Instant,
    pub is_sending: bool,
}

impl TransferSession {
    pub fn new(header: FileHeader, is_sending: bool) -> Self {
        Self {
            transfer_id: uuid::Uuid::new_v4(),
            header,
            bytes_transferred: 0,
            start_time: std::time::Instant::now(),
            is_sending,
        }
    }
    
    pub fn progress(&self) -> TransferProgress {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let transfer_rate = if elapsed > 0.0 {
            self.bytes_transferred as f64 / elapsed
        } else {
            0.0
        };
        
        let eta_seconds = if transfer_rate > 0.0 {
            Some(((self.header.size - self.bytes_transferred) as f64 / transfer_rate) as u64)
        } else {
            None
        };
        
        TransferProgress {
            bytes_transferred: self.bytes_transferred,
            total_bytes: self.header.size,
            transfer_rate,
            eta_seconds,
        }
    }
    
    pub fn update_progress(&mut self, bytes: u64) {
        self.bytes_transferred += bytes;
    }
    
    pub fn is_complete(&self) -> bool {
        self.bytes_transferred >= self.header.size
    }
}