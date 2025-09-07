//! Thumbnail generation errors

use thiserror::Error;

pub type ThumbnailResult<T> = Result<T, ThumbnailError>;

#[derive(Error, Debug)]
pub enum ThumbnailError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
    
    #[error("Video processing error: {0}")]
    VideoProcessing(String),
    
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    
    #[error("Invalid thumbnail size: {0}")]
    InvalidSize(u32),
    
    #[error("Invalid quality setting: {0} (must be 0-100)")]
    InvalidQuality(u8),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Thumbnail already exists: {0}")]
    AlreadyExists(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl ThumbnailError {
    pub fn video_processing(msg: impl Into<String>) -> Self {
        Self::VideoProcessing(msg.into())
    }
    
    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat(format.into())
    }
    
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }
    
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl From<ThumbnailError> for crate::infrastructure::jobs::error::JobError {
    fn from(err: ThumbnailError) -> Self {
        crate::infrastructure::jobs::error::JobError::execution(err.to_string())
    }
}