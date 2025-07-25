# Cross-Platform Copy Operations & Volume Awareness

## Overview

This design document addresses two critical optimizations for Core v2:

1. **Hot-swappable copy methods** - Different copy strategies based on source/destination context
2. **Volume awareness** - Integration of volume detection and management for optimal file operations

## Problem Statement

### Current Copy Implementation Issues

The current `FileCopyJob` uses basic `fs::copy()` for all operations, which:
- **Cannot leverage OS-level optimizations** (reflinks, copy-on-write)
- **Treats all copies the same** regardless of volume context  
- **No progress tracking** for byte-level operations
- **Poor performance** for cross-volume operations

### Missing Volume Context

SdPath currently stores `device_id` but lacks:
- **Volume information** for efficient routing
- **Performance characteristics** for copy strategy selection
- **Volume boundaries** for optimization decisions
- **Cross-platform volume detection**

## Research: Cross-Platform Copy Strategies

### 1. OS Reference Copies (Instant)

**Linux - `copy_file_range()` and reflinks:**
```rust
// Modern Linux kernel syscall for efficient copying
use libc::{copy_file_range, COPY_FILE_RANGE_COPY_REFLINK};

async fn copy_with_reflink(src: &Path, dst: &Path) -> Result<CopyResult, io::Error> {
    // Try reflink first (CoW filesystems like Btrfs, XFS, APFS via FUSE)
    match copy_file_range_reflink(src, dst) {
        Ok(()) => Ok(CopyResult::Reflink),
        Err(_) => {
            // Fall back to regular copy_file_range for same-filesystem
            copy_file_range_regular(src, dst).await
        }
    }
}
```

**macOS - `clonefile()` and `copyfile()`:**
```rust
use libc::{clonefile, copyfile, CLONE_NOOWNERCOPY};

async fn copy_with_clone(src: &Path, dst: &Path) -> Result<CopyResult, io::Error> {
    // APFS clone files (instant, CoW)
    if unsafe { clonefile(src_cstr, dst_cstr, CLONE_NOOWNERCOPY) } == 0 {
        Ok(CopyResult::Clone)
    } else {
        // Fall back to copyfile for optimized copying
        copy_with_copyfile(src, dst).await
    }
}
```

**Windows - `CopyFileEx()` with progress:**
```rust
use winapi::um::winbase::{CopyFileExW, COPY_FILE_NO_BUFFERING};

async fn copy_with_progress(
    src: &Path, 
    dst: &Path, 
    progress_callback: impl Fn(u64, u64)
) -> Result<CopyResult, io::Error> {
    // Native Windows copy with progress callbacks
    CopyFileExW(src, dst, Some(progress_routine), context, false, flags)
}
```

### 2. Byte Stream Copies (Progress Tracking)

For cross-volume, network, or when fine-grained progress is needed:

```rust
async fn copy_with_progress_stream(
    src: &Path,
    dst: &Path,
    progress_callback: impl Fn(u64, u64),
) -> Result<CopyResult, io::Error> {
    let mut src_file = File::open(src).await?;
    let mut dst_file = File::create(dst).await?;
    
    let total_size = src_file.metadata().await?.len();
    let mut copied = 0u64;
    let mut buffer = vec![0u8; 64 * 1024]; // 64KB chunks
    
    while copied < total_size {
        let n = src_file.read(&mut buffer).await?;
        if n == 0 { break; }
        
        dst_file.write_all(&buffer[..n]).await?;
        copied += n as u64;
        
        progress_callback(copied, total_size);
    }
    
    Ok(CopyResult::Stream { bytes_copied: copied })
}
```

## Volume System Integration

### Volume Manager Architecture

```rust
pub struct VolumeManager {
    volumes: Arc<RwLock<HashMap<VolumeFingerprint, Volume>>>,
    volume_cache: Arc<RwLock<HashMap<PathBuf, VolumeFingerprint>>>,
    event_tx: broadcast::Sender<VolumeEvent>,
}

impl VolumeManager {
    /// Get volume for a given path
    pub async fn volume_for_path(&self, path: &Path) -> Option<Volume> {
        // Check cache first
        if let Some(fingerprint) = self.volume_cache.read().await.get(path) {
            return self.volumes.read().await.get(fingerprint).cloned();
        }
        
        // Find containing volume
        let volumes = self.volumes.read().await;
        for volume in volumes.values() {
            if volume.contains_path(path) {
                // Cache the result
                self.volume_cache.write().await.insert(path.to_path_buf(), volume.fingerprint.clone().unwrap());
                return Some(volume.clone());
            }
        }
        
        None
    }
    
    /// Determine optimal copy strategy
    pub async fn optimal_copy_strategy(
        &self,
        src_path: &Path,
        dst_path: &Path,
    ) -> CopyStrategy {
        let src_volume = self.volume_for_path(src_path).await;
        let dst_volume = self.volume_for_path(dst_path).await;
        
        match (src_volume, dst_volume) {
            (Some(src), Some(dst)) if src.fingerprint == dst.fingerprint => {
                // Same volume - use OS optimizations
                self.select_same_volume_strategy(&src).await
            }
            (Some(src), Some(dst)) if self.are_volumes_equivalent(&src, &dst) => {
                // Different volumes, same device - use efficient cross-volume
                CopyStrategy::CrossVolume { 
                    use_sendfile: src.file_system.supports_sendfile(),
                    chunk_size: self.optimal_chunk_size(&src, &dst),
                }
            }
            _ => {
                // Cross-device or unknown - use safe byte stream
                CopyStrategy::ByteStream { 
                    chunk_size: 64 * 1024,
                    verify_checksum: true,
                }
            }
        }
    }
    
    async fn select_same_volume_strategy(&self, volume: &Volume) -> CopyStrategy {
        match volume.file_system {
            FileSystem::APFS => CopyStrategy::ApfsClone,
            FileSystem::EXT4 | FileSystem::Btrfs => CopyStrategy::RefLink,
            FileSystem::NTFS => CopyStrategy::NtfsClone,
            _ => CopyStrategy::SameVolumeOptimized,
        }
    }
}
```

### Volume-Aware Copy Strategies

```rust
#[derive(Debug, Clone)]
pub enum CopyStrategy {
    /// APFS clone file (instant, CoW)
    ApfsClone,
    /// Linux reflink (instant, CoW) 
    RefLink,
    /// NTFS clone (Windows, near-instant)
    NtfsClone,
    /// Same volume, optimized syscalls
    SameVolumeOptimized,
    /// Cross-volume on same device
    CrossVolume { 
        use_sendfile: bool, 
        chunk_size: usize 
    },
    /// Full byte stream copy with progress
    ByteStream { 
        chunk_size: usize, 
        verify_checksum: bool 
    },
    /// Network/cloud copy
    Network { 
        protocol: NetworkProtocol,
        compression: bool,
    },
}

#[derive(Debug, Clone)]
pub enum CopyResult {
    /// Instant clone/reflink operation
    Instant { method: String },
    /// Streamed copy with bytes transferred
    Stream { bytes_copied: u64, duration: Duration },
    /// Network transfer result
    Network { bytes_transferred: u64, speed_mbps: f64 },
}
```

## Optimized SdPath Design

### Current Issues with SdPath

```rust
// Current implementation stores device_id
#[derive(Serialize, Deserialize)]
pub struct SdPath {
    pub device_id: Uuid,        // ❌ Stored - should be computed
    pub path: PathBuf,
}
```

### Proposed Optimized SdPath

```rust
/// Core path representation - only stores essential data
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SdPath {
    /// The local path - this is the only stored data
    pub path: PathBuf,
}

/// Extended path information - computed at runtime
#[derive(Debug, Clone)]
pub struct SdPathInfo {
    pub path: SdPath,
    pub device_id: Uuid,           // Computed from current device
    pub volume: Option<Volume>,     // Computed from VolumeManager
    pub volume_fingerprint: Option<VolumeFingerprint>,
    pub is_local: bool,            // Computed
    pub exists: bool,              // Computed (cached)
}

/// Serializable version for API/storage
#[derive(Serialize, Deserialize)]
pub struct SdPathSerialized {
    pub path: PathBuf,
    // Note: device_id and volume info NOT serialized
}

impl SdPath {
    /// Create a new SdPath with just the path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            library_id: None,
        }
    }
    
    /// Get rich information about this path
    pub async fn info(&self, volume_manager: &VolumeManager) -> SdPathInfo {
        let device_id = get_current_device_id();
        let volume = volume_manager.volume_for_path(&self.path).await;
        let volume_fingerprint = volume.as_ref()
            .and_then(|v| v.fingerprint.clone());
        
        SdPathInfo {
            path: self.clone(),
            device_id,
            volume,
            volume_fingerprint,
            is_local: true, // Always true in this context
            exists: tokio::fs::metadata(&self.path).await.is_ok(),
        }
    }
    
    /// Check if this path is on the same volume as another
    pub async fn same_volume_as(
        &self, 
        other: &SdPath,
        volume_manager: &VolumeManager
    ) -> bool {
        let self_vol = volume_manager.volume_for_path(&self.path).await;
        let other_vol = volume_manager.volume_for_path(&other.path).await;
        
        match (self_vol, other_vol) {
            (Some(a), Some(b)) => a.fingerprint == b.fingerprint,
            _ => false,
        }
    }
}

/// For cross-device operations (future)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdPathRemote {
    pub device_id: Uuid,          // Required for remote paths
    pub path: PathBuf,
    pub last_known_volume: Option<VolumeFingerprint>,
}
```

### Database Integration

Store volume information in Entry/Location rather than SdPath:

```sql
-- Entries table gets volume context
ALTER TABLE entries ADD COLUMN volume_fingerprint TEXT;
ALTER TABLE entries ADD COLUMN volume_relative_path TEXT; -- Path relative to volume mount

-- Locations inherently have volume context
ALTER TABLE locations ADD COLUMN volume_fingerprint TEXT;
ALTER TABLE locations ADD COLUMN expected_volume_name TEXT;
```

## Enhanced Copy Job Implementation

### Volume-Aware Copy Job

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct FileCopyJob {
    pub sources: Vec<SdPath>,       // Now just paths
    pub destination: SdPath,
    pub options: CopyOptions,
    
    // Runtime state (not serialized)
    #[serde(skip)]
    strategy_cache: HashMap<(PathBuf, PathBuf), CopyStrategy>,
    #[serde(skip)]
    volume_manager: Option<Arc<VolumeManager>>,
}

impl FileCopyJob {
    /// Initialize with volume manager for strategy optimization
    pub fn with_volume_manager(mut self, vm: Arc<VolumeManager>) -> Self {
        self.volume_manager = Some(vm);
        self
    }
    
    async fn execute_copy(
        &mut self,
        src: &SdPath,
        dst: &SdPath,
        ctx: &JobContext<'_>,
    ) -> JobResult<CopyResult> {
        let strategy = self.get_copy_strategy(src, dst).await?;
        
        match strategy {
            CopyStrategy::ApfsClone => {
                ctx.log("Using APFS clone (instant)".to_string());
                self.execute_apfs_clone(src, dst).await
            }
            CopyStrategy::RefLink => {
                ctx.log("Using reflink (instant)".to_string());  
                self.execute_reflink(src, dst).await
            }
            CopyStrategy::ByteStream { chunk_size, verify_checksum } => {
                ctx.log(format!("Using byte stream copy ({}KB chunks)", chunk_size / 1024));
                self.execute_stream_copy(src, dst, chunk_size, verify_checksum, ctx).await
            }
            _ => {
                // Other strategies...
                self.execute_optimized_copy(src, dst, strategy, ctx).await
            }
        }
    }
    
    async fn get_copy_strategy(&mut self, src: &SdPath, dst: &SdPath) -> JobResult<CopyStrategy> {
        // Check cache first
        let cache_key = (src.path.clone(), dst.path.clone());
        if let Some(strategy) = self.strategy_cache.get(&cache_key) {
            return Ok(strategy.clone());
        }
        
        // Compute strategy
        let strategy = if let Some(vm) = &self.volume_manager {
            vm.optimal_copy_strategy(&src.path, &dst.path).await
        } else {
            // Fallback to basic strategy
            CopyStrategy::ByteStream { 
                chunk_size: 64 * 1024, 
                verify_checksum: false 
            }
        };
        
        // Cache the result
        self.strategy_cache.insert(cache_key, strategy.clone());
        Ok(strategy)
    }
    
    async fn execute_stream_copy(
        &self,
        src: &SdPath,
        dst: &SdPath,
        chunk_size: usize,
        verify_checksum: bool,
        ctx: &JobContext<'_>,
    ) -> JobResult<CopyResult> {
        let mut src_file = File::open(&src.path).await?;
        let mut dst_file = File::create(&dst.path).await?;
        
        let total_size = src_file.metadata().await?.len();
        let mut copied = 0u64;
        let mut buffer = vec![0u8; chunk_size];
        let start_time = Instant::now();
        
        // Optional checksum verification
        let mut hasher = if verify_checksum {
            Some(blake3::Hasher::new())
        } else {
            None
        };
        
        while copied < total_size {
            ctx.check_interrupt().await?;
            
            let n = src_file.read(&mut buffer).await?;
            if n == 0 { break; }
            
            dst_file.write_all(&buffer[..n]).await?;
            
            if let Some(ref mut hasher) = hasher {
                hasher.update(&buffer[..n]);
            }
            
            copied += n as u64;
            
            // Report progress every 1MB
            if copied % (1024 * 1024) == 0 {
                ctx.progress(Progress::structured(CopyProgress {
                    current_file: src.path.display().to_string(),
                    bytes_copied: copied,
                    total_bytes: total_size,
                    speed_mbps: (copied as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64(),
                    current_operation: "Streaming copy".to_string(),
                    estimated_remaining: Some(estimate_remaining_time(copied, total_size, start_time.elapsed())),
                }));
            }
        }
        
        // Verify checksum if enabled
        if let Some(hasher) = hasher {
            let src_hash = hasher.finalize();
            let dst_hash = blake3::hash(&tokio::fs::read(&dst.path).await?);
            
            if src_hash != dst_hash {
                return Err(JobError::ExecutionFailed("Checksum verification failed".to_string()));
            }
        }
        
        Ok(CopyResult::Stream { 
            bytes_copied: copied, 
            duration: start_time.elapsed() 
        })
    }
}
```

### Platform-Specific Implementations

```rust
// Platform-specific optimized copy implementations
#[cfg(target_os = "macos")]
mod macos_copy {
    use std::ffi::CString;
    use libc::{clonefile, CLONE_NOOWNERCOPY};
    
    pub async fn apfs_clone(src: &Path, dst: &Path) -> Result<CopyResult, io::Error> {
        let src_cstr = CString::new(src.to_str().unwrap())?;
        let dst_cstr = CString::new(dst.to_str().unwrap())?;
        
        let result = unsafe {
            clonefile(src_cstr.as_ptr(), dst_cstr.as_ptr(), CLONE_NOOWNERCOPY)
        };
        
        if result == 0 {
            Ok(CopyResult::Instant { method: "APFS clone".to_string() })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_copy {
    use libc::{copy_file_range, COPY_FILE_RANGE_COPY_REFLINK};
    
    pub async fn reflink_copy(src: &Path, dst: &Path) -> Result<CopyResult, io::Error> {
        // Try reflink first
        let src_fd = std::fs::File::open(src)?;
        let dst_fd = std::fs::File::create(dst)?;
        
        let result = unsafe {
            copy_file_range(
                src_fd.as_raw_fd(),
                std::ptr::null_mut(),
                dst_fd.as_raw_fd(), 
                std::ptr::null_mut(),
                usize::MAX,
                COPY_FILE_RANGE_COPY_REFLINK
            )
        };
        
        if result >= 0 {
            Ok(CopyResult::Instant { method: "reflink".to_string() })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
```

## Volume Performance Integration

### Copy Strategy Selection

```rust
impl VolumeManager {
    fn optimal_chunk_size(&self, src_volume: &Volume, dst_volume: &Volume) -> usize {
        let src_speed = src_volume.read_speed_mbps.unwrap_or(100);
        let dst_speed = dst_volume.write_speed_mbps.unwrap_or(100);
        
        // Adjust chunk size based on volume performance
        match (src_volume.disk_type, dst_volume.disk_type) {
            (DiskType::SSD, DiskType::SSD) => 1024 * 1024,      // 1MB for SSD-to-SSD
            (DiskType::HDD, DiskType::HDD) => 256 * 1024,       // 256KB for HDD-to-HDD  
            (DiskType::SSD, DiskType::HDD) => 512 * 1024,       // 512KB for mixed
            _ => 64 * 1024,                                      // 64KB default
        }
    }
    
    fn supports_reflink(&self, src_vol: &Volume, dst_vol: &Volume) -> bool {
        // Same volume with CoW filesystem
        src_vol.fingerprint == dst_vol.fingerprint && 
        matches!(src_vol.file_system, 
            FileSystem::APFS | 
            FileSystem::Btrfs | 
            FileSystem::ZFS |
            FileSystem::ReFS
        )
    }
}
```

## Implementation Plan

### Phase 1: Volume Manager Integration
1. **Port volume detection** from original core
2. **Add VolumeManager** to Core initialization
3. **Create volume fingerprinting** system
4. **Add volume caching** for path lookups

### Phase 2: SdPath Optimization  
1. **Remove device_id** from SdPath struct
2. **Add computed SdPathInfo** system
3. **Update serialization** to exclude computed fields
4. **Add volume awareness** to path operations

### Phase 3: Enhanced Copy Strategies
1. **Implement platform-specific** copy optimizations
2. **Add strategy selection** based on volume context
3. **Create progress tracking** for byte stream copies
4. **Add checksum verification** options

### Phase 4: Performance Testing
1. **Benchmark copy strategies** across different scenarios
2. **Measure volume detection** overhead
3. **Optimize chunk sizes** based on real-world performance
4. **Add performance regression** tests

## Benefits

### Performance Improvements
- **Instant copies** for same-volume operations on CoW filesystems
- **Optimized chunk sizes** based on volume performance characteristics  
- **Reduced serialization** overhead with computed fields
- **Better progress tracking** for long-running operations

### Architecture Benefits
- **Cleaner SdPath** design with separation of concerns
- **Volume-aware operations** enable smarter routing
- **Platform-specific optimizations** where available
- **Future-ready** for network and cloud operations

### User Experience
- **Faster file operations** with appropriate copy methods
- **Better progress feedback** during transfers
- **Reliable checksum verification** for important files
- **Consistent behavior** across platforms

This design provides a solid foundation for high-performance, volume-aware file operations while maintaining the clean architecture principles of Core v2.