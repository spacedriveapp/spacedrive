# Cloud Drives — Backend (Rust Core) Implementation Audit

Investigation date: 2026-04-18
Scope: Rust core, ops layer, CLI, crates, migrations, tests. No code modifications.

## 1. Executive Summary

**Verdict: Functional MVP for read/list/delete; NOT started for copy/move/rename; sync is single-shot indexing only.**

Spacedrive's cloud drive layer is a thin but real adapter over [Apache OpenDAL](https://opendal.apache.org/) 0.54. The primitive I/O surface (`read`, `read_range`, `write`, `read_dir`, `metadata`, `exists`, `delete`, `create_directory`) is fully wired through the unified `VolumeBackend` trait at `core/src/volume/backend/cloud.rs:252-441` and **every method is a real OpenDAL call** — no `todo!()`, no `unimplemented!()`, no dummy data in the cloud backend itself.

The provider matrix is broad on paper (9 services enumerated, 6 wired end-to-end) and narrow in practice: there is zero OAuth flow implementation — the user is expected to paste `access_token` + `refresh_token` pairs obtained out-of-band. Credential encryption (XChaCha20-Poly1305, library-scoped) and persistence (SQLite via SeaORM) are production-grade. Indexing works through the existing pipeline by treating cloud reads as "always new" (processing.rs:273). File copy/move/rename explicitly `panic!("Cloud storage operations are not yet implemented")` at two call sites in `core/src/ops/files/copy/job.rs`.

Tests: 2 integration tests using OpenDAL's in-memory driver (actually runnable), plus one `#[ignore]` live-S3 smoke test. No OAuth, no token-refresh, no job tests.

Bottom line: **"Partial MVP"** — the bytes-in / bytes-out pipeline is real; the user-facing workflows (auth, copy, rename, sync) are stubs or missing.

## 2. Architecture Map

### 2.1 Module tree

```
core/src/
├── volume/
│   ├── backend/
│   │   ├── mod.rs           ← VolumeBackend trait, BackendType, CloudServiceType
│   │   ├── local.rs         ← LocalBackend
│   │   └── cloud.rs         ← CloudBackend (OpenDAL-backed) ★
│   └── manager.rs           ← restore_cloud_volumes (line ~180-410),
│                              register_cloud_volume (1308),
│                              find_cloud_volume (1244),
│                              ensure_unique_mount_point (1288)
├── crypto/
│   ├── cloud_credentials.rs ← CloudCredentialManager + CloudCredential + CredentialData ★
│   └── key_manager.rs       ← library key provider (consumed)
├── infra/db/
│   ├── entities/cloud_credential.rs        ← SeaORM entity for `cloud_credentials` table
│   └── migration/
│     ├── m20251016_000001_add_cloud_identifier.rs          ← volumes.cloud_identifier
│     ├── m20251202_000001_add_cloud_config_to_volumes.rs   ← volumes.cloud_config (JSON text)
│     └── m20251204_000001_create_cloud_credentials_table.rs
├── ops/volumes/
│   ├── add_cloud/   action.rs + output.rs + mod.rs         ← register_library_action! "volumes.add_cloud"
│   └── remove_cloud/ action.rs + output.rs + mod.rs        ← register_library_action! "volumes.remove_cloud"
└── domain/
    ├── volume.rs              ← Volume struct has cloud_identifier, cloud_config, backend fields
    └── addressing.rs          ← SdPath::Cloud { service, identifier, path }

apps/cli/src/domains/
├── cloud/setup.rs   ← interactive OAuth-token paste wizard for each provider
└── volume/args.rs   ← non-interactive clap args (sd volume add-cloud)

crates/crypto/src/cloud/   ← UNRELATED: encryption primitive for future E2E cloud sync,
                             NOT tied to cloud drive providers (ChaCha20 stream cipher only)
```

### 2.2 Key types, traits, enums

| Symbol | Kind | Location |
|---|---|---|
| `VolumeBackend` trait | Interface | `core/src/volume/backend/mod.rs:27-57` |
| `BackendType { Local, Cloud(CloudServiceType) }` | Enum | `core/src/volume/backend/mod.rs:60-64` |
| `CloudServiceType` (9 variants) | Enum | `core/src/volume/backend/mod.rs:67-91` |
| `CloudBackend { operator, service_type, root }` | Struct | `core/src/volume/backend/cloud.rs:23-33` |
| `CloudCredential` | Struct | `core/src/crypto/cloud_credentials.rs:250-263` |
| `CredentialData { AccessKey, OAuth, ApiKey, ConnectionString }` | Enum | `core/src/crypto/cloud_credentials.rs:266-288` |
| `CloudCredentialManager` | Struct | `core/src/crypto/cloud_credentials.rs:43-247` |
| `CloudCredentialError` | thiserror enum | `core/src/crypto/cloud_credentials.rs:18-40` |
| `CloudStorageConfig` (6 variants, input DTO) | Enum | `core/src/ops/volumes/add_cloud/action.rs:27-74` |
| `VolumeAddCloudAction` / `VolumeRemoveCloudAction` | LibraryAction | `add_cloud/action.rs:77-505`, `remove_cloud/action.rs:22-75` |
| `SdPath::Cloud { service, identifier, path }` | Enum variant | `core/src/domain/addressing.rs:35` |

### 2.3 Database schema (from migrations)

**Table `volumes`** — two cloud columns added:

| Column | Type | Added by | Notes |
|---|---|---|---|
| `cloud_identifier` | `string NULL` | `m20251016_000001_add_cloud_identifier.rs:16` | bucket / drive root / container name |
| `cloud_config` | `text NULL` (JSON) | `m20251202_000001_add_cloud_config_to_volumes.rs:17` | region, endpoint, root — service-specific |

**Table `cloud_credentials`** (created `m20251204_000001_create_cloud_credentials_table.rs:10-53`):

| Column | Type | Notes |
|---|---|---|
| `id` | `integer PK auto-increment` | |
| `volume_fingerprint` | `string NOT NULL UNIQUE` | indexed (`idx_cloud_credentials_volume_fingerprint`) |
| `encrypted_credential` | `binary NOT NULL` | XChaCha20-Poly1305 ciphertext (24-byte nonce prepended) |
| `service_type` | `string NOT NULL` | stringified `CloudServiceType` debug format |
| `created_at` / `updated_at` | `timestamp NOT NULL` | |

Down-migration for `m20251016` is a no-op (`:24-28`) because SQLite can't drop columns easily; other two work.

### 2.4 Flow: adding a cloud volume (end-to-end)

```
CLI (apps/cli/src/domains/cloud/setup.rs)
    prompts user → builds CloudStorageConfig::S3 {...} etc.
         │
         ▼ execute_action!
    VolumeAddCloudAction::execute (add_cloud/action.rs:95-498)
         │
         ├─ match config → CloudBackend::new_s3 / new_google_drive / ... (builds OpenDAL operator)
         ├─ VolumeFingerprint::from_network_volume(backend_id, cloud_id) (domain/volume.rs:63)
         ├─ ensure_unique_mount_point("s3://bucket")
         ├─ build Volume { backend: Some(Arc<dyn VolumeBackend>), cloud_identifier, cloud_config, ... }
         ├─ CloudCredentialManager::store_credential(library_id, fingerprint, credential)
         │      → XChaCha20-Poly1305 encrypt → INSERT INTO cloud_credentials
         ├─ volume_manager.register_cloud_volume(volume)   (manager.rs:1308)
         └─ volume_manager.track_volume(library, fingerprint, name) → persist Volume row
```

On daemon startup (`manager.rs:180-409`) the flow replays in reverse: read `volumes` row → read `cloud_credentials` row → decrypt → reconstruct the appropriate `CloudBackend` → insert into `volumes` HashMap and `mount_point_cache`.

## 3. Feature-by-feature Status

| Feature | Status | Key location(s) | Notes |
|---|---|---|---|
| Authentication (OAuth flow) | **NOT STARTED** | — | User must paste tokens obtained out-of-band. No HTTP redirect server, no PKCE, no browser launch. `apps/cli/src/domains/cloud/setup.rs:161-164` literally prompts `password("Access Token")`. |
| Credential storage | **COMPLETE** | `crypto/cloud_credentials.rs:59-191` + migration `m20251204` | XChaCha20-Poly1305 with random 192-bit nonce, library-scoped key. |
| Credential retrieval/decrypt | **COMPLETE** | `cloud_credentials.rs:119-147, 221-246` | Round-trip tested (`:357-424`). |
| Token refresh | **PARTIAL (delegated)** | `cloud.rs:140-142` (comment) | OpenDAL auto-refreshes for Dropbox/GDrive/OneDrive per its own code; Spacedrive never persists the refreshed access_token back to DB. `expires_at` field exists but no code checks it. |
| Provider backend construction | **COMPLETE** (6 providers) | `cloud.rs:48-238` | S3, GoogleDrive, OneDrive, Dropbox, AzureBlob, GoogleCloudStorage. BackblazeB2/Wasabi/DigitalOceanSpaces are aliased to S3 backend in CLI (`setup.rs:87-96`), no dedicated constructor. |
| File listing / indexing | **COMPLETE (as "always new")** | `volume/backend/cloud.rs:295-341`, `ops/indexing/phases/discovery.rs:587-628`, `phases/processing.rs:270-288` | Cloud entries bypass `std::fs::symlink_metadata` change detection — every indexer pass treats cloud files as `Change::New` (see inline comment at `processing.rs:270-272`). |
| Download / read | **COMPLETE** | `cloud.rs:253-281` | Full-file and ranged reads both implemented, ranged read is the basis for sample-based content hashing (`ops/indexing/phases/content.rs:92-110`). |
| Upload / write | **COMPLETE** | `cloud.rs:283-293` | Single-shot `operator.write`. No multipart, no resumable upload, no progress. |
| Delete | **COMPLETE** | `cloud.rs:384-410`, `ops/files/delete/strategy.rs:96-230` | Detects file vs directory and uses `remove_all` for dirs. Only `DeleteMode::Permanent` allowed (`strategy.rs:104-114`); Trash/Secure rejected. |
| Rename | **NOT STARTED** | `ops/files/copy/job.rs:1042, 1485` | Hard `panic!("Cloud storage operations are not yet implemented")` on any `SdPath::Cloud`. |
| Move / Copy | **NOT STARTED** | `ops/files/copy/job.rs:1042, 1485` | Same panics. FILE-003 task is still `status: To Do` (`.tasks/core/FILE-003-cloud-volume-file-operations.md:4`). |
| Create directory | **COMPLETE** (trait) / **STUB** (action) | backend: `cloud.rs:412-432`, action: `ops/files/create_folder/action.rs:116-122` | Trait impl calls `operator.create_dir`, but `CreateFolderAction` returns `ActionError::Internal("Cloud folder creation not yet implemented")` — the two aren't wired together. |
| Sync / conflict resolution | **NOT STARTED** | — | No watcher for cloud, no ETag/version tracking. Re-indexing is the only sync mechanism. |
| Offline cache | **NOT STARTED** | — | No VFS cache, no pinning, no eviction. Reads always hit the network. |
| Streaming (range reads) | **COMPLETE** | `cloud.rs:266-281` | Used for content hashing at `content.rs:98-108`. |
| Path resolution (SdPath → DB entry) | **STUB** | `ops/indexing/path_resolver.rs:226-229` | `SdPath::Cloud { .. } => Ok(None)` with `// TODO: Implement cloud path resolution`. |
| Cloud path → backend path conversion | **S3-HARDCODED** | `ops/indexing/phases/content.rs:28-37` | `to_backend_path` strips `s3://` only — other schemes (gdrive://, dropbox://) would fall through and be passed verbatim to OpenDAL. |

## 4. Provider Support Matrix

| Provider | Enum variant | Auth fields | Backend constructor | CLI wizard | `register_cloud_volume` path | In-memory test | Live test |
|---|---|---|---|---|---|---|---|
| Amazon S3 | `S3` | AccessKey | `new_s3` (cloud.rs:48) | `add_s3_interactive` (setup.rs:73) | yes (manager.rs:213) | yes (delete_strategy_test.rs:222) | `#[ignore]` (cloud.rs:450) |
| Cloudflare R2 | `S3` (aliased) | AccessKey | via `new_s3` + endpoint | same wizard, branch 1 | via S3 path | via S3 | — |
| Backblaze B2 | `BackblazeB2` | AccessKey | **uses `new_s3`** in CLI; no dedicated constructor | setup.rs:90 | restore path falls through `_` at manager.rs:335-338 (warn+skip) | — | — |
| Wasabi | `Wasabi` | AccessKey | same as B2 | setup.rs:91 | skipped on restart | — | — |
| DigitalOcean Spaces | `DigitalOceanSpaces` | AccessKey | same as B2 | setup.rs:92 | skipped on restart | — | — |
| Google Drive | `GoogleDrive` | OAuth | `new_google_drive` (cloud.rs:77) | `add_google_drive_interactive` (setup.rs:150) | yes (manager.rs:245) | — | — |
| OneDrive | `OneDrive` | OAuth | `new_onedrive` (cloud.rs:108) | `add_onedrive_interactive` (setup.rs:190) | yes (manager.rs:265) | — | — |
| Dropbox | `Dropbox` | OAuth (refresh only) | `new_dropbox` (cloud.rs:143) | `add_dropbox_interactive` (setup.rs:230) | yes (manager.rs:285) | — | — |
| Azure Blob | `AzureBlob` | AccessKey (account name/key) | `new_azure_blob` (cloud.rs:172) | `add_azure_blob_interactive` (setup.rs:268) | yes (manager.rs:304) | — | — |
| Google Cloud Storage | `GoogleCloudStorage` | ApiKey (service-account JSON) | `new_google_cloud_storage` (cloud.rs:201) | `add_gcs_interactive` (setup.rs:303) | yes (manager.rs:322) | — | — |
| `Other` | `Other` | — | — | `setup.rs:94` (as S3 fallback) | skipped | — | — |
| WebDAV / SFTP / FTP / rclone | — | — | — | — | — | — | — |

**Critical:** Backblaze B2, Wasabi, and DigitalOcean Spaces can be **added** at runtime (CLI routes them through `new_s3`) but will **not be reloaded** after daemon restart — `manager.rs:335-338` has a catch-all that emits `warn!("Unsupported cloud service type ...")` and skips them. This is a data-loss-adjacent bug.

## 5. External Dependencies

### 5.1 Direct Cargo dependency

From `core/Cargo.toml:87-95`:

```toml
opendal = { version = "0.54", features = [
    "services-s3",
    "services-gdrive",
    "services-onedrive",
    "services-dropbox",
    "services-azblob",
    "services-gcs",
] }
```

Only these 6 OpenDAL service features are enabled. OpenDAL supports ~40 more (WebDAV, SFTP, B2-native, Swift, IPFS, Supabase, …) that Spacedrive does **not** compile in.

### 5.2 Related crypto dependency

From `core/src/crypto/cloud_credentials.rs:6-9`:

```rust
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
```

`chacha20poly1305` is transitively pulled via the workspace.

### 5.3 Protocols

All cloud I/O goes through OpenDAL, which wraps each service's native HTTPS/REST API:
- S3-family: AWS Signature v4 over HTTPS
- Google Drive / GCS: Google REST v3 with OAuth2 bearer tokens
- OneDrive: Microsoft Graph API with OAuth2 bearer tokens
- Dropbox: Dropbox HTTP API v2 with OAuth2 refresh-token flow
- Azure Blob: Azure REST API with shared-key auth

No gRPC, no direct vendor SDKs (no `aws-sdk-s3`, `google-drive3`, `azure_sdk_*`), no `rclone`/`fuser` mounts.

### 5.4 No dedicated cloud crate

- `apps/cloud/` is **commented out** in workspace root: `Cargo.toml:4` → `# "apps/cloud",`
- `sd-cloud-schema` is **commented out**: `Cargo.toml:41` → `# sd-cloud-schema = { git = ... }`
- `crates/crypto/src/cloud/` exists but is an **encryption primitive** (XChaCha20 one-shot and stream, `mod.rs:1-10`) for future end-to-end sync, **unrelated** to cloud drive providers. Stream encryption is explicitly disabled (`encrypt.rs:6-7` "temporarily disabled").
- `crates/actors/src/lib.rs:115` mentions cloud sync in a doc comment only.

## 6. Placeholder / TODO / Panic Inventory

### 6.1 Hard panics on cloud paths

| Location | Code |
|---|---|
| `core/src/ops/files/copy/job.rs:1042` | `SdPath::Cloud { .. } => panic!("Cloud storage operations are not yet implemented"),` (in `new_rename`) |
| `core/src/ops/files/copy/job.rs:1485` | `SdPath::Cloud { .. } => panic!("Cloud storage operations are not yet implemented"),` (in `rename`) |

### 6.2 Returns error at runtime

| Location | Behavior |
|---|---|
| `core/src/ops/files/create_folder/action.rs:116-122` | `SdPath::Cloud` branch returns `ActionError::Internal("Cloud folder creation not yet implemented")` despite the trait method existing. |
| `core/src/ops/files/delete/strategy.rs:104-114` | Rejects `DeleteMode::Trash` and `DeleteMode::Secure` for cloud paths with a formatted error. (Expected behavior, but worth flagging.) |
| `core/src/volume/manager.rs:335-338` | `_ =>` catch-all in restore-cloud-volumes switch emits `warn!` and `continue` for any `CloudServiceType` not explicitly handled (B2, Wasabi, DO Spaces, Other). |

### 6.3 TODO comments

| Location | Text |
|---|---|
| `core/src/ops/indexing/path_resolver.rs:227` | `// TODO: Implement cloud path resolution` (`resolve_to_entry` returns `Ok(None)` for cloud paths) |

### 6.4 Scheme handling hardcoded

| Location | Issue |
|---|---|
| `core/src/ops/indexing/phases/content.rs:28-37` | `to_backend_path` only strips `s3://`; other cloud URIs pass through unchanged and may confuse non-S3 backends during content hashing. |

### 6.5 Clean zones (zero todo!/unimplemented!/FIXME)

Verified zero placeholder markers in:
- `core/src/volume/backend/cloud.rs` (485 lines)
- `core/src/volume/backend/mod.rs` (150 lines)
- `core/src/crypto/cloud_credentials.rs` (452 lines)
- `core/src/infra/db/entities/cloud_credential.rs` (26 lines)
- `core/src/ops/volumes/add_cloud/*` and `core/src/ops/volumes/remove_cloud/*`
- All three cloud migrations

The cloud **backend itself** is not riddled with stubs. The stubs live in the **callers** (copy/rename jobs, path resolver).

## 7. Tests Coverage

### 7.1 Inventory

| Test | Location | Status | Notes |
|---|---|---|---|
| `test_cloud_backend_s3` | `core/src/volume/backend/cloud.rs:450-484` | `#[ignore]` | Live AWS smoke test (needs env vars). |
| `test_encrypt_decrypt_credential` | `core/src/crypto/cloud_credentials.rs:356-424` | runs | Round-trip XChaCha20 encrypt/decrypt against a real SQLite DB. |
| `test_credential_expiry` | `core/src/crypto/cloud_credentials.rs:426-451` | runs | `is_expired()` boundary checks. |
| `test_cloud_backend_delete_file` | `core/tests/delete_strategy_test.rs:221-247` | runs | Uses `opendal::services::Memory` — real `VolumeBackend` calls. |
| `test_cloud_backend_delete_directory` | `core/tests/delete_strategy_test.rs:249-299` | runs | Recursive delete via `remove_all`, in-memory. |

### 7.2 Missing coverage

- No test for `read` / `read_range` / `write` / `read_dir` / `metadata` / `create_directory` paths through `VolumeBackend`.
- No `VolumeAddCloudAction` / `VolumeRemoveCloudAction` action-level test.
- No test of `VolumeManager::restore_cloud_volumes` (the daemon-restart path).
- No test of `find_cloud_volume` mount-point cache semantics.
- No OAuth test (expected, since there is no OAuth implementation).
- No indexer-end-to-end test exercising cloud discovery + content hashing.
- No test for B2/Wasabi/DO Spaces round-trip (which would surface the `_ =>` skip bug on restart).

## 8. Critical Gaps (MVP-blocking vs cosmetic)

### 8.1 MVP-blocking

1. **OAuth flow** — `apps/cli/src/domains/cloud/setup.rs:161-164` asking users to paste access tokens is a non-starter for a consumer product. Needs a loopback HTTP listener or device-code flow per provider.
2. **Copy/move/rename `panic!`** — `ops/files/copy/job.rs:1042, 1485` make any rename or move on a cloud path a hard crash rather than a graceful error. Either implement `CloudCopyStrategy` (FILE-003 is To Do) or return `ActionError` for now.
3. **B2 / Wasabi / DO Spaces don't survive daemon restart** — `manager.rs:335-338` skips them. The "add" path works, the "restore after restart" path doesn't. Data isn't lost (credentials still in DB) but the volume disappears from the UI.
4. **`create_folder` disconnect** — backend trait method works (`cloud.rs:412-432`) but `CreateFolderAction` returns error for cloud paths. Trivial to wire (`action.rs:116-122`).
5. **Token refresh persistence** — OpenDAL refreshes in-memory; Spacedrive never writes the new `access_token` back. When the daemon restarts, it loads the stale token and relies on OpenDAL to refresh again on first request. Works in practice as long as the refresh token stays valid. No test proves this.
6. **`path_resolver.rs:226-229`** — cloud path → DB entry lookup always returns `None`, which silently breaks any query that tries to resolve a cloud `SdPath` to its `entry::Model` (e.g., for tagging, notes, labels).

### 8.2 Cosmetic / future-work

7. No offline cache, no VFS mount, no prefetch — every read is a network round-trip. Fine for MVP, becomes painful with >10k files.
8. Change detection for cloud is "always new" (`processing.rs:270-288`). Correct but wasteful — re-hashes every file on every indexer pass. ETag/LastModified integration is a known future enhancement per the inline comment.
9. `to_backend_path` only strips `s3://` (`content.rs:28-37`). Other schemes silently pass through; works only because OpenDAL is forgiving, fragile by design.
10. No `supports_block_cloning`, `total_capacity`, `available_space` discovery for cloud volumes — all hardcoded to `0` / `false` at `add_cloud/action.rs:433-434, 459`.
11. Provider enum includes `BackblazeB2`, `Wasabi`, `DigitalOceanSpaces`, `Other` with `scheme()` entries but no dedicated constructor on `CloudBackend` and no CLI-wizard mapping that preserves them through restart.
12. No WebDAV, SFTP, FTP, IPFS, Supabase Storage — OpenDAL can do all these; features just aren't enabled.

### 8.3 Task-tracker alignment

- `.tasks/core/CLOUD-003-cloud-volume.md` — status `In Progress`. Acceptance criteria: 2 of 3 boxes checked; "Files can be copied to and from the cloud volume" is `[ ]`. Accurate.
- `.tasks/core/FILE-003-cloud-volume-file-operations.md` — status `To Do`. All 6 acceptance criteria unchecked. Accurate.
- `.tasks/core/VOL-004-remote-volume-indexing-with-opendal.md` — related; claims parity with CLOUD-003.

---

## Appendix A — `VolumeBackend` trait completeness (cloud vs local)

| Method | `CloudBackend` | `LocalBackend` | Notes |
|---|---|---|---|
| `read` | ✓ `cloud.rs:253` | ✓ `local.rs` | both real |
| `read_range` | ✓ `cloud.rs:266` | ✓ | both real |
| `write` | ✓ `cloud.rs:283` | ✓ | no progress/multipart on cloud |
| `read_dir` | ✓ `cloud.rs:295` | ✓ | cloud returns `inode: None` always (`:336`) |
| `metadata` | ✓ `cloud.rs:343` | ✓ | cloud returns `created/accessed/inode/permissions: None` (`:368-371`) |
| `exists` | ✓ `cloud.rs:375` | ✓ | cloud implements via `stat()` fallback — 2 HTTP calls on hot path |
| `delete` | ✓ `cloud.rs:384` | ✓ | cloud auto-detects dir vs file |
| `create_directory` | ✓ `cloud.rs:412` | ✓ | `recursive` param ignored on cloud |
| `is_local` | ✓ `false` | ✓ `true` | |
| `backend_type` | ✓ `BackendType::Cloud(_)` | ✓ `BackendType::Local` | |

Parity: **100%** of the trait surface is implemented in both backends. There are no cloud-only or local-only methods.

## Appendix B — Cloud service URI schemes (from `CloudServiceType::scheme()`)

| Service | Scheme | Mount-point format example |
|---|---|---|
| S3 | `s3` | `s3://my-bucket` |
| GoogleDrive | `gdrive` | `gdrive://root` |
| OneDrive | `onedrive` | `onedrive://root` |
| Dropbox | `dropbox` | `dropbox://root` |
| AzureBlob | `azblob` | `azblob://my-container` |
| GoogleCloudStorage | `gcs` | `gcs://my-bucket` |
| BackblazeB2 | `b2` | (parsed but not constructible on restart) |
| Wasabi | `wasabi` | (same) |
| DigitalOceanSpaces | `spaces` | (same) |
| Other | `cloud` | catch-all fallback |

These schemes are used for `find_cloud_volume` cache keys (`manager.rs:1250`) and `SdPath::Cloud` parsing (`addressing.rs:636-659`).
