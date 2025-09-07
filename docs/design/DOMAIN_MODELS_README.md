# Domain Models

The domain layer contains the core business entities that power Spacedrive's Virtual Distributed File System (VDFS).

## Core Models

### Entry
The foundation of the VDFS. Represents any file or directory that Spacedrive knows about.

```rust
let entry = Entry {
    id: Uuid::new_v4(),
    sd_path: SdPathSerialized { device_id, path },
    name: "vacation.jpg",
    kind: EntryKind::File { extension: Some("jpg") },
    metadata_id: metadata.id,  // ALWAYS has metadata!
    content_id: None,          // Optional - for deduplication
    // ...
};
```

Key features:
- Uses `SdPath` for cross-device addressing
- Always has `UserMetadata` (can tag any file immediately)
- `ContentIdentity` is optional (progressive enhancement)

### UserMetadata
Decoupled from content, enabling immediate tagging of any file.

```rust
let mut metadata = UserMetadata::new(entry.metadata_id);
metadata.add_tag(Tag {
    name: "Vacation",
    color: Some("#FF6B6B"),
    icon: Some("🏖️"),
});
metadata.favorite = true;
```

### ContentIdentity
Optional component for deduplication and content-based features.

```rust
let content = ContentIdentity::new(cas_id, CURRENT_CAS_VERSION);
content.kind = ContentKind::Image;
content.media_data = Some(MediaData { width: 3000, height: 2000, ... });
```

### Location
An indexed directory that Spacedrive monitors.

```rust
let location = Location::new(
    library_id,
    "My Documents",
    SdPathSerialized::from_sdpath(&SdPath::local("/Users/me/Documents")),
    IndexMode::Deep,
);
```

### Device
Unified concept replacing the old Node/Device/Instance confusion.

```rust
let device = Device::current();
// "MacBook Pro", macOS, online, etc.
```

## Key Relationships

```
Entry (file/dir)
  ├─ sd_path: SdPathSerialized (cross-device path)
  ├─ metadata_id → UserMetadata (ALWAYS exists)
  └─ content_id → ContentIdentity (optional)

Location (indexed directory)
  └─ sd_path: SdPathSerialized (can be on any device)

Device (machine running Spacedrive)
  └─ Referenced by SdPath for routing operations
```

## Design Benefits

1. **Immediate Tagging**: Any file can be tagged without content indexing
2. **Cross-Device Operations**: SdPath enables true VDFS
3. **Progressive Enhancement**: Start simple, add features as needed
4. **Content Changes**: Metadata persists when files are edited
5. **Clean Separation**: User data vs content identity

## Usage Example

```rust
// Discover a file
let entry = Entry::new(
    SdPath::local("/Users/me/photo.jpg"),
    metadata
);

// Tag it immediately (no content indexing required!)
let mut user_meta = UserMetadata::new(entry.metadata_id);
user_meta.add_tag(vacation_tag);

// Later, index content for deduplication
let content = ContentIdentity::new(generate_cas_id(&entry).await?, 2);
entry.content_id = Some(content.id);

// Copy to another device with metadata
let dest = SdPath::new(iphone_id, "/Photos/Vacation");
copy_with_metadata(entry.sd_path(), dest).await?;
```

This architecture enables Spacedrive's promise of a true Virtual Distributed File System!