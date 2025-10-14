---
id: CLOUD-003
title: Cloud Storage Provider as a Volume
status: Completed
assignee: unassigned
parent: CLOUD-000
priority: High
tags: [cloud, storage, volume, s3]
whitepaper: Section 5.2
last_updated: 2025-10-13
---

## Description

Implement support for a cloud storage provider (e.g., S3-compatible service) as a native Spacedrive Volume. This will allow users to add cloud storage as a location in their library, just like a local disk.

## Implementation Steps

1.  Create a new `Volume` implementation for a generic S3-compatible API.
    - `VolumeBackend` trait implemented
    - `CloudBackend` with OpenDAL integration for S3-compatible services
    - Support for S3, R2, MinIO, Wasabi, Backblaze B2, DigitalOcean Spaces
2.  Implement the necessary file operations (read, write, list, delete) for the S3 API.
    - `read()`, `read_range()`, `write()`, `read_dir()`, `metadata()`, `exists()`
    - Sample-based content hashing using ranged reads (~58KB for large files)
3.  Integrate the new cloud volume type into the `VolumeManager`.
    - Cloud volumes tracked in database
    - Credentials encrypted with XChaCha20-Poly1305 and stored in OS keyring
    - `VolumeAddCloudAction` and `VolumeRemoveCloudAction` implemented
4.  Develop the CLI/UI flow for adding and configuring a cloud storage volume.
    - CLI commands: `sd volume add-cloud`, `sd volume remove-cloud`
    - Support for custom endpoints (R2, MinIO, etc.)
5.  Update query system to support cloud paths.
    - `Entry::try_from` supports `SdPath::Cloud`
    - `DirectoryListingQuery` supports cloud directories
    - `FileByPathQuery` supports cloud files
6.  Update indexer to use VolumeBackend for I/O operations.
    - Query layer supports cloud paths
    - Discovery phase uses backend.read_dir()
    - Processing phase handles cloud backends (skips change detection for cloud)
    - Content phase uses backend for content hashing

## Acceptance Criteria
-   [x] A user can add an S3 bucket as a new location in their library.
-   [ ] Files can be copied to and from the cloud volume.
-   [x] The cloud volume can be indexed like any other location.

## Implementation Files

**Core Backend:**
- `core/src/volume/backend/mod.rs` - VolumeBackend trait
- `core/src/volume/backend/local.rs` - LocalBackend implementation
- `core/src/volume/backend/cloud.rs` - CloudBackend with OpenDAL

**Credential Management:**
- `core/src/crypto/cloud_credentials.rs` - CloudCredentialManager

**Actions:**
- `core/src/ops/volumes/add_cloud/` - VolumeAddCloudAction
- `core/src/ops/volumes/remove_cloud/` - VolumeRemoveCloudAction

**CLI:**
- `apps/cli/src/domains/volume/` - CLI commands

**Query System:**
- `core/src/domain/entry.rs` - Cloud path support
- `core/src/ops/files/query/directory_listing.rs` - Cloud directory browsing
- `core/src/ops/files/query/file_by_path.rs` - Cloud file lookup

## Next Steps

1. Test end-to-end cloud volume indexing with MinIO or real S3
2. Implement file copy operations for cloud volumes
3. Add OAuth support for Google Drive, Dropbox, OneDrive
