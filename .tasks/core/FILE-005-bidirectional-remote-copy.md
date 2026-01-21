---
id: FILE-005
title: Bidirectional Remote File Copy (PULL Support)
status: Done
assignee: jamiepine
parent: FILE-000
priority: High
tags: [core, jobs, file-ops, networking, p2p]
---

## Description

Extend the FileCopyJob to support bidirectional file transfers, enabling files to be copied FROM remote devices TO the local device (PULL operations). Currently, only PUSH operations work (local → remote), causing "Source must be local path" errors when attempting to drag files from remote device explorers to local folders.

## Problem Statement

**Current State:**

- ✅ PUSH: `local://device-a/file.jpg` → `local://device-b/dest/` (works)
- ❌ PULL: `local://device-b/file.jpg` → `local://device-a/dest/` (fails with "Source must be local path")

**User Experience Gap:**
When browsing a remote device's file tree (synced via library metadata), users can see files but cannot drag them to local folders. The job is created locally but fails because the RemoteTransferStrategy requires the source to be local.

## Architecture

### Current RemoteTransferStrategy (PUSH Only)

```rust
// strategy.rs:285-287
let local_path = source
    .as_local_path()
    .ok_or_else(|| anyhow::anyhow!("Source must be local path"))?;

// Reads local file, streams to remote destination
```

Flow:

```
Device A (local):  Has file, creates job
                   ↓
                   Connects to Device B (remote destination)
                   ↓
                   Streams file chunks → Device B
```

### Proposed Bidirectional Strategy (PUSH + PULL)

Detect transfer direction and use appropriate protocol:

```rust
pub enum TransferDirection {
    Push,  // Local source → Remote destination (existing)
    Pull,  // Remote source → Local destination (new)
}

impl RemoteTransferStrategy {
    fn detect_direction(source: &SdPath, dest: &SdPath) -> TransferDirection {
        match (source.is_local_to_current_device(), dest.is_local_to_current_device()) {
            (true, false) => TransferDirection::Push,
            (false, true) => TransferDirection::Pull,
            _ => panic!("Invalid cross-device configuration"),
        }
    }
}
```

### PULL Protocol Flow

```
Device A (local):  Wants file, creates job
                   Source: local://device-b/path/file.jpg
                   Dest:   local://device-a/Desktop/
                   ↓
                   Resolves device-b slug → UUID
                   ↓
                   Connects to Device B via Iroh
                   ↓
                   Sends PullRequest message:
                   {
                       transfer_id: Uuid,
                       source_path: "/path/file.jpg",
                       requested_by: device_a_id,
                   }
                   ↓
Device B (remote): Receives PullRequest
                   ↓
                   Validates path access (security)
                   ↓
                   Reads local file, calculates checksum
                   ↓
                   Streams file chunks back to Device A
                   ↓
Device A (local):  Receives chunks, writes to destination
                   ↓
                   Verifies final checksum
                   ↓
                   Sends PullComplete acknowledgment
```

## Implementation Phases

### Phase 1: Protocol Extension

**Files:**

- `core/src/service/network/protocol/file_transfer.rs`

**Changes:**

1. Add new message types to `FileTransferMessage` enum:

   ```rust
   pub enum FileTransferMessage {
       // Existing PUSH messages
       TransferRequest { ... },
       FileChunk { ... },
       TransferComplete { ... },

       // New PULL messages
       PullRequest {
           transfer_id: Uuid,
           source_path: PathBuf,
           requested_by: Uuid,  // Requesting device ID
       },
       PullResponse {
           transfer_id: Uuid,
           file_metadata: FileMetadata,
           accepted: bool,
           error: Option<String>,
       },
       // Reuse FileChunk and TransferComplete for actual transfer
   }
   ```

2. Add PULL request handler to protocol implementation
3. Add security validation for path access (prevent directory traversal, respect library boundaries)

### Phase 2: Strategy Refactor

**Files:**

- `core/src/ops/files/copy/strategy.rs`

**Changes:**

1. Refactor `RemoteTransferStrategy::execute()` to detect direction:

   ```rust
   async fn execute(&self, ctx: &JobContext, source: &SdPath, dest: &SdPath, ...) -> Result<()> {
       let direction = Self::detect_direction(source, dest, ctx)?;

       match direction {
           TransferDirection::Push => self.execute_push(ctx, source, dest, ...).await,
           TransferDirection::Pull => self.execute_pull(ctx, source, dest, ...).await,
       }
   }
   ```

2. Extract current logic into `execute_push()` (minimal refactor)

3. Implement new `execute_pull()`:

   ```rust
   async fn execute_pull(
       &self,
       ctx: &JobContext,
       source: &SdPath,  // Remote path
       dest: &SdPath,    // Local path
       verify_checksum: bool,
       progress_callback: Option<&ProgressCallback>,
   ) -> Result<()> {
       // 1. Extract remote device and path
       let (source_device_slug, source_path) = source.as_physical()
           .ok_or_else(|| anyhow::anyhow!("Source must be physical path"))?;

       let local_dest_path = dest.as_local_path()
           .ok_or_else(|| anyhow::anyhow!("Destination must be local path"))?;

       // 2. Resolve remote device
       let library = ctx.library();
       let source_device_id = library.resolve_device_slug(source_device_slug)?;

       // 3. Connect to remote device
       let networking = ctx.networking_service()?;
       let endpoint = networking.endpoint();
       let device_registry = networking.device_registry();
       let remote_node_id = device_registry.get_node_id(&source_device_id)?;

       // 4. Initiate PULL request
       let transfer_id = Uuid::new_v4();
       let connection = endpoint.connect(remote_node_id, FILE_TRANSFER_ALPN).await?;
       let (mut send, mut recv) = connection.open_bi().await?;

       // 5. Send PullRequest message
       let request = FileTransferMessage::PullRequest {
           transfer_id,
           source_path: source_path.to_path_buf(),
           requested_by: library.device_id(),
       };
       send_message(&mut send, &request).await?;

       // 6. Receive PullResponse
       let response: FileTransferMessage = recv_message(&mut recv).await?;
       let file_metadata = match response {
           FileTransferMessage::PullResponse { accepted: true, file_metadata, .. } => file_metadata,
           FileTransferMessage::PullResponse { accepted: false, error, .. } => {
               return Err(anyhow::anyhow!("Pull request rejected: {}", error.unwrap_or_default()));
           }
           _ => return Err(anyhow::anyhow!("Unexpected response to pull request")),
       };

       // 7. Receive file chunks and write locally
       let mut file = tokio::fs::File::create(local_dest_path).await?;
       let mut hasher = blake3::Hasher::new();
       let mut total_bytes_received = 0u64;

       loop {
           let msg: FileTransferMessage = recv_message(&mut recv).await?;

           match msg {
               FileTransferMessage::FileChunk { chunk_index, data, chunk_checksum, .. } => {
                   // Verify chunk checksum
                   let calculated = blake3::hash(&data);
                   if calculated.as_bytes() != &chunk_checksum {
                       return Err(anyhow::anyhow!("Chunk {} checksum mismatch", chunk_index));
                   }

                   // Write chunk
                   file.write_all(&data).await?;
                   hasher.update(&data);
                   total_bytes_received += data.len() as u64;

                   // Progress callback
                   if let Some(cb) = progress_callback {
                       cb(total_bytes_received, file_metadata.size);
                   }

                   // Send ack
                   send_message(&mut send, &FileTransferMessage::ChunkAck {
                       transfer_id,
                       chunk_index,
                   }).await?;
               }
               FileTransferMessage::TransferComplete { final_checksum, total_bytes, .. } => {
                   // Verify final checksum
                   if verify_checksum {
                       let calculated = hasher.finalize();
                       if calculated.as_bytes() != &final_checksum {
                           return Err(anyhow::anyhow!("Final checksum mismatch"));
                       }
                   }

                   if total_bytes != total_bytes_received {
                       return Err(anyhow::anyhow!("Byte count mismatch"));
                   }

                   break;
               }
               _ => return Err(anyhow::anyhow!("Unexpected message during transfer")),
           }
       }

       file.flush().await?;

       ctx.log(format!(
           "Successfully pulled {} ({} bytes) from device:{}",
           source_path.display(),
           total_bytes_received,
           source_device_slug
       ));

       Ok(())
   }
   ```

### Phase 3: Protocol Handler Implementation

**Files:**

- `core/src/service/network/protocol/file_transfer.rs`

**Changes:**

1. Add `handle_pull_request()` method:

   ```rust
   async fn handle_pull_request(
       &self,
       transfer_id: Uuid,
       source_path: PathBuf,
       requested_by: Uuid,
       send: &mut SendStream,
       recv: &mut RecvStream,
   ) -> Result<()> {
       // 1. Security validation
       if !self.validate_path_access(&source_path, requested_by).await? {
           let response = FileTransferMessage::PullResponse {
               transfer_id,
               file_metadata: Default::default(),
               accepted: false,
               error: Some("Access denied".to_string()),
           };
           send_message(send, &response).await?;
           return Ok(());
       }

       // 2. Get file metadata
       let metadata = tokio::fs::metadata(&source_path).await?;
       let file_metadata = FileMetadata {
           size: metadata.len(),
           modified: metadata.modified()?.into(),
           created: metadata.created()?.into(),
       };

       // 3. Calculate checksum
       let checksum = calculate_file_checksum(&source_path).await?;

       // 4. Send acceptance response
       let response = FileTransferMessage::PullResponse {
           transfer_id,
           file_metadata: file_metadata.clone(),
           accepted: true,
           error: None,
       };
       send_message(send, &response).await?;

       // 5. Stream file chunks (reuse existing stream_file_data logic)
       self.stream_file_to_remote(
           transfer_id,
           &source_path,
           &file_metadata,
           checksum,
           send,
           recv,
       ).await?;

       Ok(())
   }
   ```

2. Add path validation:

   ```rust
   async fn validate_path_access(&self, path: &Path, requested_by: Uuid) -> Result<bool> {
       // Check if path is within a library location
       // Prevent directory traversal attacks
       // Verify requesting device is trusted/paired
       // Respect indexer rules (don't expose ignored paths)

       // TODO: Implement proper authorization
       Ok(true)
   }
   ```

3. Register PULL message handlers in protocol router

### Phase 4: Testing

**Files:**

- `core/tests/file_copy_pull.rs` (new)

**Test Cases:**

1. **Basic PULL**: Copy single file from remote device to local
2. **Large file PULL**: Test chunked transfer with progress
3. **Checksum verification**: Verify Blake3 validation on PULL
4. **Resume PULL**: Test interrupted PULL resumes correctly
5. **Security**: Test path traversal prevention, access control
6. **Error handling**: Network disconnection, file not found, permission denied
7. **Concurrent PULLs**: Multiple files from same/different devices

### Phase 5: UI Integration

**Files:**

- `packages/interface/src/app/$libraryId/Explorer/DragAndDrop.tsx`

**Changes:**

- Remove/update any UI-level blocking for remote files
- Ensure drag-from-remote-to-local triggers FileCopyJob correctly
- Update error messages to be more helpful

## Security Considerations

1. **Path Validation**: Prevent directory traversal attacks
   - Normalize paths, reject `..` components
   - Ensure paths are within library-managed locations

2. **Access Control**: Only allow PULL from trusted devices
   - Verify requesting device is paired
   - Check library membership
   - Respect indexer rules (don't expose .gitignore'd files)

3. **Rate Limiting**: Prevent DoS via excessive PULL requests
   - Track concurrent transfers per device
   - Implement backpressure

4. **Audit Logging**: Log all PULL requests for security review
   - Who requested what, when
   - Success/failure outcomes

## Performance Considerations

1. **Checksum Optimization**: For PULL, remote device calculates checksum
   - Saves local CPU, but increases initial latency
   - Consider making checksum optional for trusted devices

2. **Chunk Size**: 64KB chunks work well for PUSH, should be same for PULL
   - Balanced between throughput and responsiveness

3. **Connection Reuse**: Iroh connections should be pooled
   - Multiple files in single job can reuse connection

4. **Resume Support**: PULL should support resume like PUSH
   - Track completed chunks in job state
   - On reconnect, request remaining chunks only

## Edge Cases

1. **File Modified During Transfer**: Remote file changes while streaming
   - Final checksum will fail (correct behavior)
   - User should retry

2. **Offline Device**: Source device not connected
   - Fail fast with "Device offline" error
   - Consider queuing for later (future work)

3. **Simultaneous Bidirectional Transfer**: Device A pulls from B while B pulls from A
   - Should work independently (no deadlock)
   - Each device handles both as separate transfers

4. **Cross-Cloud PULL**: Pull from cloud via remote device
   - Remote device proxies from cloud to local device
   - May be slow (cloud → remote → local)
   - Future: Direct cloud pull optimization

## Acceptance Criteria

- [x] PULL operations work: can copy file from remote device to local
- [x] Security: Path traversal attacks are prevented
- [x] Security: Only paired devices can request PULL
- [x] Checksums are verified end-to-end (Blake3)
- [x] Progress reporting works for PULL (byte-level updates)
- [ ] Resume works for interrupted PULL transfers
- [x] Error messages are clear (offline device, permission denied, file not found)
- [x] UI drag-and-drop from remote device to local folder works seamlessly
- [x] Integration tests cover basic PULL, large files, errors, and resume
- [ ] PULL performance is comparable to PUSH (same throughput)

## Related Tasks

- FILE-001: File Copy Job with Strategy Pattern (parent implementation)
- NET-001: Iroh P2P Stack (networking foundation)
- LSYNC-010: Sync Service (library metadata sync that enables remote browsing)

## Implementation Files

- `core/src/ops/files/copy/strategy.rs` - Add execute_pull() method
- `core/src/service/network/protocol/file_transfer.rs` - Add PULL protocol messages and handlers
- `core/tests/file_copy_pull.rs` - Integration tests
- `packages/interface/src/app/$libraryId/Explorer/DragAndDrop.tsx` - UI updates
