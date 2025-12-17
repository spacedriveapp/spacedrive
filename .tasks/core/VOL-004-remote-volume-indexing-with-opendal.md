---
id: VOL-004
title: Cloud Volume Indexing with OpenDAL
status: Done
assignee: jamiepine
parent: VOL-000
priority: High
tags: [volume, remote-indexing, opendal, cloud]
whitepaper: Section 4.3.5
last_updated: 2025-10-14
related_tasks: [CLOUD-003]
---

## Description

Integrate the OpenDAL library to enable indexing of remote storage services like S3, FTP, and SMB as native Spacedrive Volumes.

**Note**: This task is being implemented in conjunction with CLOUD-003 (Cloud Storage Provider as a Volume).

## Implementation Steps

1.  Integrate the `opendal` crate into the project.
    - Added to `core/Cargo.toml` with features for S3, GCS, Azure, etc.
2.  Create a new `Volume` implementation that uses OpenDAL as a backend.
    - `CloudBackend` struct wraps `opendal::Operator`
    - Implements `VolumeBackend` trait for unified I/O abstraction
3.  Implement the necessary file operations (read, write, list, delete) using the OpenDAL API.
    - `read()`, `read_range()` for efficient content hashing
    - `read_dir()` for directory traversal
    - `metadata()` for file stats
    - `write()`, `delete()` for file operations
4.  Integrate the new remote volume type into the `VolumeManager` and the indexing process.
    - VolumeManager integration complete
    - Query system supports remote paths
    - Indexer uses VolumeBackend abstraction for all I/O operations
5.  Develop the CLI/UI flow for adding and configuring a remote storage volume.
    - CLI: `sd volume add-cloud` and `sd volume remove-cloud`
    - Secure credential storage in OS keyring

## Acceptance Criteria

- [x] A user can add a remote storage service as a new location in their library.
- [x] Files on the remote storage can be indexed and browsed like any other location.
- [x] The system can handle authentication and configuration for different remote services.

## Currently Supported Services

**S3-Compatible (via OpenDAL):**

- Amazon S3
- Cloudflare R2
- MinIO (self-hosted)
- Wasabi
- Backblaze B2
- DigitalOcean Spaces

**Planned:**

- Google Drive (OAuth required)
- Dropbox (OAuth required)
- OneDrive (OAuth required)
- Google Cloud Storage
- Azure Blob Storage
- FTP/SFTP

## Implementation References

See CLOUD-003 for detailed implementation files and architecture.

## Next Steps

1. Test end-to-end indexing with various OpenDAL services
2. Add OAuth support for consumer cloud services (Drive, Dropbox, OneDrive)
3. Performance testing and optimization for remote I/O operations
