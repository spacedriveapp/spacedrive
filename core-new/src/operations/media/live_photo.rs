//! Live Photo detection and handling
//!
//! When enabled, Live Photos are handled as follows:
//! 1. During indexing, when we encounter an image file (HEIC/JPEG), we check for a matching video (MOV/MP4)
//! 2. If found, the video becomes a virtual sidecar of the image
//! 3. The video file is NOT indexed as a separate entry - it only exists as a sidecar
//! 4. This prevents duplicate processing and keeps Live Photos as single logical units

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a detected Live Photo pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePhoto {
    /// The image component (HEIC/JPEG)
    pub image_path: PathBuf,
    /// The video component (MOV/MP4)
    pub video_path: PathBuf,
    /// Optional asset identifier if found in metadata
    pub asset_id: Option<String>,
}

/// Detects Live Photo pairs based on naming conventions and timestamps
pub struct LivePhotoDetector;

impl LivePhotoDetector {
    /// Known Live Photo patterns:
    /// 1. Apple Photos exports: `IMG_1234.HEIC` + `IMG_1234.MOV`
    /// 2. iCloud Photos: `photo.heic` + `photo.mov` (same name, different extension)
    /// 3. Some apps: `photo.jpg` + `photo.mp4`
    pub fn detect_pair(path: &Path) -> Option<LivePhoto> {
        let file_name = path.file_stem()?.to_str()?;
        let extension = path.extension()?.to_str()?.to_lowercase();
        
        // Check if this is an image file
        let is_image = matches!(extension.as_str(), "heic" | "heif" | "jpg" | "jpeg");
        let is_video = matches!(extension.as_str(), "mov" | "mp4");
        
        if !is_image && !is_video {
            return None;
        }
        
        let parent = path.parent()?;
        
        // Define the counterpart we're looking for
        let counterpart_extensions = if is_image {
            vec!["mov", "mp4"]
        } else {
            vec!["heic", "heif", "jpg", "jpeg"]
        };
        
        // Look for matching counterpart
        for ext in counterpart_extensions {
            let counterpart_path = parent.join(format!("{}.{}", file_name, ext));
            if counterpart_path.exists() && counterpart_path != path {
                // Found a match!
                let (image_path, video_path) = if is_image {
                    (path.to_path_buf(), counterpart_path)
                } else {
                    (counterpart_path, path.to_path_buf())
                };
                
                return Some(LivePhoto {
                    image_path,
                    video_path,
                    asset_id: None, // Could be extracted from EXIF/metadata later
                });
            }
        }
        
        None
    }
    
    /// Check if two files form a Live Photo pair
    pub fn is_live_photo_pair(image_path: &Path, video_path: &Path) -> bool {
        // Must be in same directory
        if image_path.parent() != video_path.parent() {
            return false;
        }
        
        // Must have same base name
        if image_path.file_stem() != video_path.file_stem() {
            return false;
        }
        
        // Check extensions
        let img_ext = image_path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
            
        let vid_ext = video_path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        let valid_image = matches!(img_ext.as_str(), "heic" | "heif" | "jpg" | "jpeg");
        let valid_video = matches!(vid_ext.as_str(), "mov" | "mp4");
        
        valid_image && valid_video
    }
    
    /// Generate a deterministic UUID for a Live Photo pair
    /// This ensures both components reference the same Live Photo ID
    pub fn generate_live_photo_id(image_hash: &str, video_hash: &str) -> Uuid {
        // Use the smaller hash first for deterministic ordering
        let (first, second) = if image_hash < video_hash {
            (image_hash, video_hash)
        } else {
            (video_hash, image_hash)
        };
        
        let combined = format!("{}-{}", first, second);
        
        // Use a namespace UUID for Live Photos
        const LIVE_PHOTO_NAMESPACE: Uuid = Uuid::from_bytes([
            0x4c, 0x69, 0x76, 0x65, 0x50, 0x68, 0x6f, 0x74,
            0x6f, 0x4e, 0x53, 0x00, 0x00, 0x00, 0x00, 0x01,
        ]);
        
        Uuid::new_v5(&LIVE_PHOTO_NAMESPACE, combined.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_live_photo_detection() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();
        
        // Create test files
        let image_path = dir_path.join("IMG_1234.HEIC");
        let video_path = dir_path.join("IMG_1234.MOV");
        
        fs::write(&image_path, b"fake image").unwrap();
        fs::write(&video_path, b"fake video").unwrap();
        
        // Test detection from image
        let result = LivePhotoDetector::detect_pair(&image_path);
        assert!(result.is_some());
        let live_photo = result.unwrap();
        assert_eq!(live_photo.image_path, image_path);
        assert_eq!(live_photo.video_path, video_path);
        
        // Test detection from video
        let result = LivePhotoDetector::detect_pair(&video_path);
        assert!(result.is_some());
        let live_photo = result.unwrap();
        assert_eq!(live_photo.image_path, image_path);
        assert_eq!(live_photo.video_path, video_path);
        
        // Test pair validation
        assert!(LivePhotoDetector::is_live_photo_pair(&image_path, &video_path));
    }
    
    #[test]
    fn test_live_photo_id_generation() {
        let id1 = LivePhotoDetector::generate_live_photo_id("hash1", "hash2");
        let id2 = LivePhotoDetector::generate_live_photo_id("hash2", "hash1");
        
        // Should generate same ID regardless of order
        assert_eq!(id1, id2);
    }
}