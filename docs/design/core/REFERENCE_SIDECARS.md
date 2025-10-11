<!--CREATED: 2025-08-11-->
# Reference Sidecars Implementation

This document describes the reference sidecar feature added to the Virtual Sidecar System (VSS).

## Overview

Reference sidecars allow Spacedrive to track files as virtual sidecars without moving them from their original locations. This aligns with Spacedrive's philosophy of not touching original files during indexing.

## Key Features

1. **Non-Destructive Tracking**: Files remain in their original locations
2. **Database Linking**: Sidecars are linked to their source entries via `source_entry_id`
3. **Bulk Conversion**: Reference sidecars can be converted to owned sidecars on demand

## Database Schema

Added to the `sidecars` table:
- `source_entry_id: Option<i32>` - Links to the original entry when the sidecar is a reference

## Implementation

### Creating Reference Sidecars

```rust
sidecar_manager.create_reference_sidecar(
    library,
    content_uuid,     // The content this is a sidecar for
    source_entry_id,  // The entry ID of the original file
    kind,
    variant,
    format,
    size,
    checksum,
).await?;
```

### Converting to Owned Sidecars

```rust
sidecar_manager.convert_reference_to_owned(
    library,
    content_uuid,
).await?;
```

This method:
1. Finds all reference sidecars for the content
2. Moves files to the managed sidecar directory
3. Updates database records to remove the reference

## Live Photo Use Case

Live Photos are the primary use case for reference sidecars:

1. During indexing, when an image is found with a matching video
2. The video is created as a reference sidecar of the image
3. The video file stays in its original location
4. Users can later bulk-convert Live Photos to take ownership

### Example Flow

```rust
// During indexing
if let Some(live_photo) = LivePhotoDetector::detect_pair(image_path) {
    // Create minimal entry for video (or skip entirely)
    let video_entry_id = create_minimal_entry(&live_photo.video_path)?;
    
    // Create reference sidecar
    LivePhotoDetector::create_live_photo_reference_sidecar(
        library,
        sidecar_manager,
        &image_content_uuid,
        video_entry_id,
        video_size,
        video_checksum,
    ).await?;
}
```

## Benefits

1. **Preserves User Organization**: Files stay where users put them
2. **Delayed Decision**: Users can choose when/if to consolidate files
3. **Reduced Indexing Impact**: No file moves during initial scan
4. **Flexibility**: Supports various sidecar relationships without file ownership