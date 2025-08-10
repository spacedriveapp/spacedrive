use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::types::{SidecarKind, SidecarVariant, SidecarFormat};

/// Represents a fully computed sidecar path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarPath {
    /// Relative path from the sidecars directory
    pub relative_path: PathBuf,
    /// Full absolute path
    pub absolute_path: PathBuf,
    /// Shard directories (h0, h1)
    pub shards: (String, String),
}

/// Builder for computing sidecar paths with sharding
pub struct SidecarPathBuilder {
    library_path: PathBuf,
}

impl SidecarPathBuilder {
    /// Create a new path builder for a library
    pub fn new(library_path: impl AsRef<Path>) -> Self {
        Self {
            library_path: library_path.as_ref().to_path_buf(),
        }
    }
    
    /// Compute the shard directories from a content UUID
    /// Returns (h0, h1) where h0 and h1 are the first two byte-pairs
    /// of the canonical, lowercase hex UUID with hyphens removed
    pub fn compute_shards(content_uuid: &Uuid) -> (String, String) {
        // Convert UUID to lowercase hex string without hyphens
        let hex = content_uuid.simple().to_string().to_lowercase();
        
        // Extract first two byte-pairs
        let h0 = hex[0..2].to_string();
        let h1 = hex[2..4].to_string();
        
        (h0, h1)
    }
    
    /// Build a sidecar path
    pub fn build(
        &self,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
    ) -> SidecarPath {
        let (h0, h1) = Self::compute_shards(content_uuid);
        
        // Build relative path: content/{h0}/{h1}/{content_uuid}/{kind_dir}/{variant}.{ext}
        let mut relative_path = PathBuf::from("content");
        relative_path.push(&h0);
        relative_path.push(&h1);
        relative_path.push(content_uuid.to_string());
        relative_path.push(kind.directory());
        
        // Filename is variant.extension
        let filename = format!("{}.{}", variant.as_str(), format.extension());
        relative_path.push(filename);
        
        // Build absolute path
        let mut absolute_path = self.library_path.clone();
        absolute_path.push("sidecars");
        absolute_path.push(&relative_path);
        
        SidecarPath {
            relative_path,
            absolute_path,
            shards: (h0, h1),
        }
    }
    
    /// Build a path to the content directory for a given UUID
    pub fn build_content_dir(&self, content_uuid: &Uuid) -> PathBuf {
        let (h0, h1) = Self::compute_shards(content_uuid);
        
        let mut path = self.library_path.clone();
        path.push("sidecars");
        path.push("content");
        path.push(&h0);
        path.push(&h1);
        path.push(content_uuid.to_string());
        
        path
    }
    
    /// Build a path to the manifest file for a given content UUID
    pub fn build_manifest_path(&self, content_uuid: &Uuid) -> PathBuf {
        let mut path = self.build_content_dir(content_uuid);
        path.push("manifest.json");
        path
    }
    
    /// Get the base sidecars directory for the library
    pub fn sidecars_dir(&self) -> PathBuf {
        let mut path = self.library_path.clone();
        path.push("sidecars");
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_shards() {
        let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
        let (h0, h1) = SidecarPathBuilder::compute_shards(&uuid);
        assert_eq!(h0, "ab");
        assert_eq!(h1, "cd");
    }
    
    #[test]
    fn test_build_path() {
        let library_path = PathBuf::from("/tmp/test.sdlibrary");
        let builder = SidecarPathBuilder::new(&library_path);
        
        let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
        let kind = SidecarKind::Thumb;
        let variant = SidecarVariant::new("grid@2x");
        let format = SidecarFormat::Webp;
        
        let path = builder.build(&uuid, &kind, &variant, &format);
        
        assert_eq!(
            path.relative_path,
            PathBuf::from("content/ab/cd/abcd1234-5678-90ab-cdef-123456789012/thumbs/grid@2x.webp")
        );
        
        assert_eq!(
            path.absolute_path,
            PathBuf::from("/tmp/test.sdlibrary/sidecars/content/ab/cd/abcd1234-5678-90ab-cdef-123456789012/thumbs/grid@2x.webp")
        );
        
        assert_eq!(path.shards, ("ab".to_string(), "cd".to_string()));
    }
    
    #[test]
    fn test_build_manifest_path() {
        let library_path = PathBuf::from("/tmp/test.sdlibrary");
        let builder = SidecarPathBuilder::new(&library_path);
        
        let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
        let manifest_path = builder.build_manifest_path(&uuid);
        
        assert_eq!(
            manifest_path,
            PathBuf::from("/tmp/test.sdlibrary/sidecars/content/ab/cd/abcd1234-5678-90ab-cdef-123456789012/manifest.json")
        );
    }
}