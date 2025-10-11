<!--CREATED: 2025-06-25-->
# Spacedrive v2: Integration System Design (Revised)

## Overview

The Spacedrive Integration System enables third-party extensions to seamlessly integrate with Spacedrive's core functionality. The system is designed from the ground up to support direct interaction with third-party services, most notably enabling the **direct, remote indexing of large-scale cloud storage** without requiring a local sync. It also supports custom file type handlers, search extensions, and lazy content processors, all while maintaining security, performance, and reliability.

## Design Principles

### 1\. Process Isolation

- Each integration runs as a separate, sandboxed process.
- The Spacedrive core remains stable and secure, even if an integration crashes or misbehaves.
- Resource usage can be monitored and limited on a per-integration basis.

### 2\. Language Agnostic

- Integrations can be written in any language, encouraging broader community contribution.
- Communication is handled via standard, high-performance IPC protocols.

### 3\. On-Demand Data Access

- The system is built to avoid local synchronization of cloud storage.
- Metadata and content are fetched on-demand from remote sources, enabling the management of petabyte-scale libraries on devices with limited local storage.

### 4\. Unified Core Logic

- The core indexer's advanced logic (change detection, batching, aggregation, database operations) is reused for all storage locations, whether local or remote.
- Integrations act as "data providers" rather than implementing their own indexing or sync logic.

## Architecture Overview

The architecture treats integrations as isolated data providers. The core communicates with them to request metadata and content on demand.

```
┌─────────────────────────────────────────────────────────┐
│                    Spacedrive Core                      │
│  ┌─────────────────┐  ┌──────────────────────────────┐  │
│  │ Integration     │  │         Core Systems         │  │
│  │ Manager         │  │  • Location Manager          │  │
│  │                 │  │  • Indexer & Job System      │  │
│  │ • Registry      │  │  • File Type Registry       │  │
│  │ • Lifecycle Mgmt│  │  • Event Bus                │  │
│  │ • IPC Router    │  │  • Credential Manager       │  │
│  │ • Sandbox       │  └──────────────────────────────┘  │
│  └─────────────────┘                                    │
└─────────────────────────────────────────────────────────┘
              │  (IPC: Metadata & Content Requests) │
              └──────────────┬────────────────────────────┘
                             │
            ┌────────────────▼────────────────┐
            │  (Isolated Integration Process) │
            │ ┌─────────────────────────────┐ │
            │ │   Integration Main Logic    │ │
            │ │ (e.g., Google Drive Plugin) │ │
            │ └─────────────┬─────────────┘ │
            │               │ (Uses OpenDAL)│
            │ ┌─────────────▼─────────────┐ │
            │ │      OpenDAL Operator     │ │
            │ └─────────────────────────────┘ │
            └────────────────┬────────────────┘
                             │ (Native API Calls)
                             ▼
                    [ Third-Party API ]
                    (e.g., Google Drive)
```

## The Remote Indexing & Content Fetching Model

This model is central to the design. It ensures Spacedrive can handle massive cloud locations efficiently.

**1. Remote Discovery:**

- When indexing a cloud location, the core `IndexerJob` dispatches a request to the appropriate integration, asking it to discover the contents of a path.
- The integration process uses a library like **Apache OpenDAL** to list files and folders directly from the cloud API (e.g., S3, Google Drive).
- The integration translates the API response into the standard `DirEntry` format and streams this metadata back to the core. **File content is not downloaded at this stage.**
- The core indexer's `Processing` phase consumes these `DirEntry` objects as if they came from the local filesystem, reusing all its database and change-detection logic.

**2. On-Demand Content Hashing:**

- During the `ContentIdentification` phase, the indexer needs to generate a content hash (`cas_id`) for each file.
- For a remote file, the indexer requests specific byte ranges from the integration (e.g., the first 8KB, three 10KB samples, and the last 8KB).
- The integration uses OpenDAL to perform efficient ranged requests to the cloud API, fetching only the required data chunks.
- These chunks are streamed back to the core and fed into the hasher. This allows hashing of terabyte-scale files with minimal bandwidth.

**3. Lazy Thumbnail & Rich Metadata Extraction:**

- After the main index is complete, a separate, lower-priority `ThumbnailerJob` is dispatched for visual media files.
- This job requests the **full file content** (or relevant portions, like headers for EXIF data) from the integration on-demand.
- This lazy processing ensures the UI is responsive and the initial index is fast, with rich media populating in the background.

## Core Components

The core components like `IntegrationManager`, `IntegrationRegistry`, `IpcRouter`, and `CredentialManager` remain largely as defined in the original design document, as they provide a robust foundation for managing isolated processes.

## Integration Types

The traits defining integration capabilities are revised to support the on-demand model.

### Cloud Storage Provider

This is the primary integration type for storage.

```rust
#[async_trait]
pub trait CloudStorageProvider {
    /// Discover entries at a given remote path.
    /// This should be a stream to handle very large directories.
    async fn discover(&self, path: &str, credentials: &IntegrationCredential) -> Result<Stream<DirEntry>>;

    /// Stream the content of a remote file.
    /// The implementation should support efficient byte range requests.
    async fn stream_content(
        &self,
        path: &str,
        range: Option<ByteRange>,
        credentials: &IntegrationCredential,
    ) -> Result<Stream<Bytes>>;

    // ... other methods for writing/managing files (create_folder, write_file, etc.)
}
```

## Job System Integration

The job system is updated to defer heavy processing.

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IntegrationJob {
    pub integration_id: String,
    pub operation: IntegrationOperation,
    pub params: JsonValue,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IntegrationOperation {
    /// Generates a thumbnail for a specific entry.
    ThumbnailGeneration {
        entry_id: i32,
        // The path/location info would be looked up from the entry_id
    },
    /// Extracts rich metadata like EXIF, video duration, etc.
    MetadataExtraction {
        entry_id: i32,
    },
    // ... other integration-specific background tasks
}

// Example Handler for the ThumbnailerJob
#[async_trait]
impl JobHandler for IntegrationJob {
    type Output = JobOutput;

    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        match &self.operation {
            IntegrationOperation::ThumbnailGeneration { entry_id } => {
                // 1. Get entry details from DB, including its remote path and integration_id
                let entry = ctx.db().find_entry_by_id(*entry_id).await?;

                // 2. Request the full file stream from the integration
                let file_stream = IntegrationManager::request_content_stream(
                    &self.integration_id,
                    &entry.remote_path,
                    None // No range, we need the whole file (or enough for thumbnailing)
                ).await?;

                // 3. Process the stream with a thumbnailing library
                let thumbnail_data = generate_thumbnail_from_stream(file_stream).await?;

                // 4. Save the thumbnail data back to the database, linked to the entry
                ctx.db().save_thumbnail(*entry_id, thumbnail_data).await?;

                Ok(JobOutput::Success)
            }
            _ => todo!("Other operations")
        }
    }
}
```

## Location System Integration

Adding a cloud location now configures it for remote indexing instead of local sync.

```rust
impl LocationManager {
    /// Add cloud storage location
    pub async fn add_cloud_location(
        &self,
        integration_id: &str,
        // ... other params like credentials_id, name
    ) -> Result<Uuid> {
        // 1. Create a virtual device for the cloud service.
        let virtual_device_id = self.device_manager.create_virtual_device(...).await?;

        // 2. Create the location record in the database.
        //    Crucially, it is marked with the integration_id.
        let location = ManagedLocation {
            // ...
            device_id: virtual_device_id,
            integration_id: Some(integration_id.to_string()),
            // ...
        };
        library.save_location(&location).await?;

        // 3. The location is now ready. An IndexerJob can be dispatched on it.
        //    The JobManager will see the `integration_id` and use the remote
        //    discovery mechanism instead of the local one.
        //    (No `CloudSync` job is needed).

        Ok(location.id)
    }
}
```

## Implementation Phases (Revised)

### Phase 1: Foundation (3-4 weeks)

- [ ] Integration manager, IPC, Process Lifecycle, Registry, Credential Management.
- [ ] **Modify `IndexerJob` to be generic over a `Discovery` mechanism.**
- [ ] Implement `LocalDiscovery` using existing filesystem logic.

### Phase 2: Remote Discovery & Content (4-5 weeks)

- [ ] **Define the `CloudStorageProvider` trait with `discover` and `stream_content`.**
- [ ] Build a proof-of-concept integration (e.g., for S3) using **OpenDAL**.
- [ ] Implement the IPC logic for streaming `DirEntry` metadata and file content bytes.
- [ ] Adapt the `IndexerJob` to handle remote discovery and on-demand content hashing.

### Phase 3: Lazy Jobs & File Types (3-4 weeks)

- [ ] Implement the `ThumbnailerJob` and `MetadataExtractionJob` as `IntegrationJob` types.
- [ ] Implement the `FileTypeHandler` interface for custom metadata and thumbnail generation hooks.

### Phase 4 & 5: Advanced Features & DX (Unchanged)

- [ ] Search Provider, Security Hardening, SDK, Documentation, etc..
