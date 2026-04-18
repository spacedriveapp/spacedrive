# Cloud Drives Integration — Design & Vision Report

**Investigation baseline:** 2026-04-18
**Scope:** Design intent for integrating Google Drive, OneDrive, Dropbox, S3, and other cloud services into Spacedrive V2.
**Method:** Pure reading of `.tasks/`, `docs/`, whitepaper references. No code read in this pass.

---

## 1. Executive Summary

Cloud drives are positioned as a **first-class, foundational feature** in Spacedrive V2 — not a peripheral integration. They sit at the heart of the Virtual Distributed File System (VDFS) premise: "one interface to manage files across all devices and clouds" (`docs/overview/philosophy.mdx:26`).

Three distinct "cloud" concepts coexist in the codebase. They must not be conflated:

1. **Cloud-as-a-Volume** (CLOUD-003 / VOL-004): Third-party cloud storage (S3, R2, Google Drive, Dropbox, etc.) mounted as a native Spacedrive volume via OpenDAL. The user's data lives in the provider's cloud; Spacedrive indexes and operates on it.
2. **Cloud-as-a-Peer** (CLOUD-000 / CLOUD-001 / CLOUD-002): A Spacedrive-managed cloud service where an isolated `sd-core` instance runs for a user in Spacedrive's own cloud infrastructure and participates in the library sync network as if it were just another device.
3. **Cloud credential vault** (SEC-005): OS-keyring-backed encrypted storage for any credentials the above two features need.

The **unified design principle** is that every file — whether on a local SSD, an external disk, a paired peer, or in S3 — shares a single addressing scheme (`SdPath`), a single content-identity hash, and a single operation surface. Cloud is not a bolt-on; it is **rendered indistinguishable from every other storage backend at the abstraction boundary** (`VolumeBackend` trait).

### Key design principles

- **"Cloud as a peer"** (CLOUD-000): Spacedrive's own managed backend is architecturally a peer in the P2P sync network, not a privileged central server. Same protocols, same code paths. This is called out as "a cornerstone of the project's unique hybrid architecture" (`.tasks/core/CLOUD-000-cloud-as-a-peer.md:13`).
- **"Cloud as a volume"**: Third-party cloud services are mounted as volumes that behave identically to local disks for indexing, search, and file operations (`docs/core/cloud-integration.mdx:6-12`).
- **Service-native addressing**: URIs match industry tools (`s3://bucket/key`, `gdrive://My Drive/...`) so paths copy-paste into AWS CLI, gsutil, etc. without modification (`docs/core/addressing.mdx:51-54`).
- **Local-first, not cloud-forced**: The V1 post-mortem explicitly calls out "Cloud focus" as a business-model mistake; V2 pivots to premium extensions, and cloud services are "optional" (`docs/overview/history.mdx:114`, `docs/overview/history.mdx:218`, `docs/overview/history.mdx:287`).
- **Content identity is backend-agnostic**: Sample-based hashing (~58KB transferred for files >100KB) guarantees a photo on Google Drive and its copy on your laptop share the same content hash, enabling true cross-storage dedup (`docs/core/cloud-integration.mdx:94-113`).

---

## 2. Supported / Planned Providers

### Currently integrated (S3 family, fully tested — CLOUD-003 / VOL-004 status "Done"/"In Progress")

From `.tasks/core/VOL-004-remote-volume-indexing-with-opendal.md:48-55` and `docs/core/volumes.mdx:265-277`:

- Amazon S3
- Cloudflare R2
- MinIO (self-hosted)
- Wasabi
- Backblaze B2
- DigitalOcean Spaces

All six use the same `s3://` scheme with an optional `endpoint` override (`docs/core/volumes.mdx:314-319`).

### Implemented with OpenDAL backend but not production-tested

From `docs/core/volumes.mdx:270-275`:

- Google Drive
- Dropbox
- OneDrive
- Google Cloud Storage
- Azure Blob Storage

### Referenced / planned but not implemented

From `docs/core/cloud-integration.mdx:82-92` (the "all major providers" vision statement) and scattered task notes:

- **Object Storage**: S3, Azure Blob, Google Cloud Storage, MinIO, Backblaze B2, Cloudflare R2
- **Consumer Cloud**: Google Drive, Dropbox, OneDrive, iCloud Drive, pCloud, MEGA
- **Enterprise Storage**: SharePoint, Box, Nextcloud, Seafile, WebDAV
- **Regional Services**: Alibaba Cloud OSS, Tencent COS, Huawei OBS, Baidu BOS
- **Protocol-level**: FTP/SFTP, SMB (`.tasks/core/VOL-004-remote-volume-indexing-with-opendal.md:16` mentions FTP and SMB)

The "40+ cloud services via OpenDAL" figure (`docs/core/volumes.mdx:75`) is an aspirational umbrella — the actual compile-time surface is whatever OpenDAL features are enabled in `core/Cargo.toml`.

### Priority order (inferred from task status and "Next Steps")

1. **S3-family** — Done / In Progress (CLOUD-003 acceptance criteria partially met).
2. **Google Drive, Dropbox, OneDrive** — OpenDAL backend wired, blocked on **native OAuth 2.0 with PKCE** (`docs/core/volumes.mdx:277`, `docs/core/volumes.mdx:385`, `docs/core/cloud-integration.mdx:374`).
3. **GCS, Azure Blob** — Backend ready, likely low-effort follow-on.
4. **Everything else** — unscheduled.

### Authentication strategy per provider

| Category | Current state | Planned |
|---|---|---|
| S3 / S3-compatible | Access key + secret key + region + optional endpoint. Entered manually via CLI or action input. | No change planned — key-pair auth is native to S3. |
| GCS / Azure Blob | Service-specific creds (service account JSON, account key) — manual input. | Unchanged. |
| Google Drive / Dropbox / OneDrive | "Manual credential setup" — user has to obtain tokens out-of-band. | **OAuth 2.0 with PKCE** (`docs/core/volumes.mdx:385`). Token refresh is NOT YET IMPLEMENTED; users must manually re-auth on expiry (`docs/core/cloud-integration.mdx:374`). |
| WebDAV / Nextcloud / SharePoint / Box | Not implemented | Presumably basic auth or OAuth per protocol. |
| FTP/SFTP/SMB | Not implemented | Username/password or key-based. |

All credentials flow into the **CloudCredentialManager** which encrypts them with XChaCha20-Poly1305 using a per-library key, then stores the encrypted blob (see §3).

---

## 3. Architecture Design

### 3.1 How cloud integrates with the core architecture

Cloud integration plugs into four existing V2 subsystems rather than creating a parallel stack:

```
┌────────────────────────────────────────────────────────────────┐
│                     SdPath (universal addressing)              │
│   Physical { device_slug, path } | Cloud { service, id, path } │
│                       | Content { uuid }                       │
└──────────────────────────────┬─────────────────────────────────┘
                               │
┌──────────────────────────────┴─────────────────────────────────┐
│                   VolumeManager  +  Volume (db entity)         │
│   - mount_point cache (O(1) URI → volume_fingerprint)          │
│   - cloud_identifier field on Volume row                       │
└──────────────────────────────┬─────────────────────────────────┘
                               │
┌──────────────────────────────┴─────────────────────────────────┐
│             VolumeBackend trait (abstraction)                  │
│  read / read_range / write / read_dir / metadata / exists      │
│  delete / create_directory                                     │
└───────────┬──────────────────────────────────┬─────────────────┘
            │                                  │
┌───────────┴───────────┐              ┌──────┴──────────────────┐
│     LocalBackend      │              │    CloudBackend          │
│  (tokio::fs wrapper)  │              │   (wraps OpenDAL::       │
│                       │              │    Operator)             │
└───────────────────────┘              └──────────────────────────┘
                                                │
                                  ┌─────────────┴─────────────┐
                                  │        OpenDAL            │
                                  │  S3 | GCS | Azure | GDrive│
                                  │  Dropbox | OneDrive | ... │
                                  └───────────────────────────┘
```

**Key entry points** (from `.tasks/core/CLOUD-003-cloud-volume.md:49-74`):

- `core/src/volume/backend/mod.rs` — `VolumeBackend` trait
- `core/src/volume/backend/local.rs` — `LocalBackend`
- `core/src/volume/backend/cloud.rs` — `CloudBackend` (OpenDAL)
- `core/src/crypto/cloud_credentials.rs` — `CloudCredentialManager`
- `core/src/ops/volumes/add_cloud/` — `VolumeAddCloudAction`
- `core/src/ops/volumes/remove_cloud/` — `VolumeRemoveCloudAction`

### 3.2 SdPath — the universal abstraction

From `docs/core/data-model.mdx:35-62` and `docs/core/cloud-integration.mdx:60-74`:

```rust
pub enum SdPath {
    Physical {
        device_slug: String,
        path: PathBuf,
    },
    Cloud {
        service: CloudServiceType,   // S3, GoogleDrive, OneDrive, ...
        identifier: String,          // bucket / drive name / container
        path: String,                // cloud-native key path
    },
    Content {
        content_id: Uuid,
    },
    Sidecar {
        content_id: Uuid,
        kind: SidecarKind,
        variant: SidecarVariant,
        format: SidecarFormat,
    },
}
```

Cloud paths are **self-contained**: the service type and identifier are embedded in the enum, so a `SdPath::Cloud` can be serialised/transported without a separate volume registry lookup for parsing. Volume resolution (URI → fingerprint) still happens through the `VolumeManager` mount-point cache (`docs/core/cloud-integration.mdx:76`).

### 3.3 Volume entity — cloud representation

From `docs/core/data-model.mdx:381-415`:

```rust
pub struct Volume {
    pub id: i32,
    pub uuid: Uuid,
    pub device_id: Uuid,              // FK — which device "owns" this cloud volume
    pub fingerprint: String,          // stable identifier across mounts
    pub display_name: Option<String>,
    pub mount_point: Option<String>,  // e.g. "s3://my-bucket"
    pub file_system: Option<String>,
    // ... capacity, performance, classification ...
    pub is_network_drive: Option<bool>,
    pub cloud_identifier: Option<String>,  // ← cloud-specific field
    pub tracked_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub is_online: bool,
}
```

A cloud volume is a normal `Volume` row with `cloud_identifier` populated. Its `device_id` is the **local device that added it** — which means a cloud volume's "owning device" follows the physical machine that configured it, not a virtual cloud peer. This is a subtle but important design decision for sync ownership (`docs/core/volumes.mdx:36-41`).

### 3.4 Data flow

#### Adding a cloud volume

From `docs/core/cloud-integration.mdx:17-32`:

```rust
let input = VolumeAddCloudInput {
    service: CloudServiceType::S3,
    display_name: "My Bucket".to_string(),
    config: CloudStorageConfig::S3 {
        bucket: "my-bucket", region: "us-west-2",
        access_key_id: "...", secret_access_key: "...",
        endpoint: None,
    },
};
dispatcher.execute_library_action::<VolumeAddCloudAction>(input, ctx).await?;
```

1. `VolumeAddCloudAction` validates config.
2. `CloudCredentialManager` encrypts config with the library's XChaCha20-Poly1305 key.
3. Encrypted blob stored (library DB row keyed by volume fingerprint, with the key itself in the OS keyring via `KeyManager`).
4. New `Volume` row created with `cloud_identifier` and `mount_point` = `s3://bucket`.
5. `VolumeManager` mount-point cache updated for O(1) resolution.

#### Indexing a cloud volume

From `docs/core/cloud-integration.mdx:133-165` and `.tasks/core/CLOUD-003-cloud-volume.md:37-41`:

- **Discovery phase** — `backend.read_dir()` uses OpenDAL's native `list` on the provider.
- **Processing phase** — "Cloud entries are treated as new during initial indexing since metadata-based change detection is not yet implemented." Cloud paths explicitly **skip change detection** (`.tasks/core/CLOUD-003-cloud-volume.md:40`).
- **Content phase** — Sample-based hashing via `backend.read_range()`; transfers only ~58KB regardless of file size (`docs/core/cloud-integration.mdx:94-113`).
- **Aggregation phase** — Directory sizes computed as usual.

Index modes `Shallow` and `Content` work identically on cloud volumes (`docs/core/cloud-integration.mdx:147-165`).

#### File operations

From `docs/core/cloud-integration.mdx:167-213`:

- **Local → Cloud / Cloud → Local** — `FileCopyJob` streams via the `VolumeBackend`.
- **Cloud → Cloud (same volume)** — uses native server-side copy "when available".
- **Cloud → Cloud (cross-provider)** — streams via local system.
- **Deletion** — `LocalDeleteStrategy` routes cloud paths through `VolumeBackend.delete()`; no separate `CloudDeleteStrategy`. Implemented via `OpenDAL::delete`/`remove_all` (`.tasks/core/FSYNC-001-delete-strategy-pattern.md:155-161`).
- **Rename** — Cloud-aware but stubbed in `CloudBackend` (`.tasks/core/FILE-004-rename-and-folders.md:38`).

#### Sync, conflicts

- **No real-time file watcher** on cloud volumes (`docs/core/cloud-integration.mdx:372`).
- **No change detection** against cloud metadata yet (`docs/core/cloud-integration.mdx:370`).
- Conflict handling is manual: "Manually refresh the cloud volume / Re-index affected directories / Check cloud provider's version history / Resolve conflicts through the UI" (`docs/core/cloud-integration.mdx:354-361`).
- A `<Tip>` hints at "cloud provider webhooks when available for real-time change notifications" — aspirational (`docs/core/cloud-integration.mdx:363-366`).

### 3.5 Key abstractions (summary)

| Abstraction | Location | Purpose |
|---|---|---|
| `SdPath::Cloud` | `core/src/domain/addressing/` | Uniform addressing — no cloud-specific call sites outside backend. |
| `VolumeBackend` trait | `core/src/volume/backend/mod.rs` | I/O abstraction shared by local and cloud. |
| `CloudBackend` | `core/src/volume/backend/cloud.rs` | Wraps `opendal::Operator`. |
| `CloudServiceType` enum | (domain) | Tag for provider variant. |
| `CloudStorageConfig` enum | `core/src/ops/volumes/add_cloud/` | Per-provider config payload. |
| `CloudCredentialManager` | `core/src/crypto/cloud_credentials.rs` | Encrypts / decrypts / stores credentials. |
| `VolumeAddCloudAction` / `VolumeRemoveCloudAction` | `core/src/ops/volumes/` | User-facing action registry. |

### 3.6 Credential storage — in depth

From `docs/core/key-manager.mdx:88-101` and `docs/core/volumes.mdx:279-285`:

```rust
pub struct CloudCredential {
    volume_fingerprint: String,
    encrypted_credential: Vec<u8>,  // encrypted with per-library key
    service_type: String,           // "google_drive", "dropbox", etc.
}
```

Hierarchy:

```
OS Keychain (Keychain / Credential Manager / Secret Service)
  └─ Device Key (256-bit root)
      └─ Library Keys (XChaCha20-Poly1305)
          └─ Cloud Credentials (stored in library DB, encrypted)
```

Security properties (`docs/core/volumes.mdx:280-285`, `docs/core/cloud-integration.mdx:115-131`):

- Encrypted with **library-specific** keys — isolation between libraries.
- Bound to both `library_id` **and** `volume_fingerprint`.
- Never written to disk in plaintext.
- Automatically deleted when the cloud volume is removed.
- Uses AEAD (XChaCha20-Poly1305, 24-byte nonce, Poly1305 MAC).

Migration note (`docs/core/key-manager.mdx:296-304`): cloud credentials used to sit directly in the OS keychain ("unreliable"); they have been consolidated into the unified `KeyManager` with redb.

---

## 4. Task Breakdown

### CLOUD-000 — Epic: Cloud as a Peer

- **Status:** To Do
- **Assignee:** jamiepine
- **Priority:** High
- **Whitepaper:** Section 5
- **Objective:** Build the "Native Cloud Service" — Spacedrive's own managed backend runs isolated `sd-core` instances per user that participate as peers in the library sync network.
- **Implementation steps:** None listed (epic only, 13-line file).
- **Acceptance criteria:** None listed.
- **Children:** CLOUD-001, CLOUD-002, CLOUD-003.

> Quote (`.tasks/core/CLOUD-000-cloud-as-a-peer.md:13`): "This epic covers the implementation of the 'Native Cloud Service' vision, where the cloud backend is treated as just another Spacedrive peer. This is a cornerstone of the project's unique hybrid architecture, enabling seamless integration between local and cloud resources."

---

### CLOUD-001 — Design Managed Cloud Core Infrastructure

- **Status:** To Do
- **Assignee:** jamiepine
- **Priority:** High
- **Whitepaper:** Section 5.1
- **Parent:** CLOUD-000
- **Objective:** Design the infrastructure for provisioning and running isolated `sd-core` instances per user in a cloud environment (likely Kubernetes).
- **Implementation steps:**
  1. ☐ Research and select cloud technologies (Kubernetes, Docker, serverless).
  2. ☐ Design multi-tenant isolation architecture.
  3. ☐ Define provisioning/de-provisioning lifecycle.
  4. ☐ Specify security and networking policies.
- **Acceptance criteria:**
  - ☐ Architecture document created.
  - ☐ Addresses scalability, security, cost.
  - ☐ Design approved.
- **Completion:** 0 / 3 criteria. Pure design stage.

---

### CLOUD-002 — Asynchronous Relay Server

- **Status:** To Do
- **Assignee:** jamiepine
- **Priority:** High
- **Whitepaper:** Section 5.3
- **Parent:** CLOUD-000
- **Objective:** Standalone relay server enabling async P2P communication when peers are offline — critical for shareable links and asynchronous Spacedrop.
- **Implementation steps:**
  1. ☐ Develop standalone relay server application.
  2. ☐ Store-and-forward for offline peers.
  3. ☐ Integrate with core networking service.
  4. ☐ Fallback to relay when direct P2P fails.
- **Acceptance criteria:**
  - ☐ Relay runs as standalone service.
  - ☐ Two peers communicate async through relay.
  - ☐ Graceful fallback when direct connection fails.
- **Completion:** 0 / 3 criteria.
- **Dependencies:** NET-000 / NET-001 (Iroh P2P stack).

---

### CLOUD-003 — Cloud Storage Provider as a Volume

- **Status:** In Progress (`last_updated: 2025-10-14`)
- **Assignee:** jamiepine
- **Priority:** High
- **Whitepaper:** Section 5.2
- **Parent:** CLOUD-000
- **Objective:** Mount a cloud storage provider (S3-compatible to start) as a native Spacedrive Volume with full indexing and file-operation support.
- **Implementation steps** (from `.tasks/core/CLOUD-003-cloud-volume.md:17-41`):
  1. ☑ `VolumeBackend` trait + `CloudBackend` with OpenDAL for S3-compatible services (S3, R2, MinIO, Wasabi, Backblaze B2, DigitalOcean Spaces).
  2. ☑ Read/write/list/delete: `read()`, `read_range()`, `write()`, `read_dir()`, `metadata()`, `exists()`, sample-based hashing.
  3. ☑ Integrated into `VolumeManager`; credentials encrypted + `VolumeAddCloudAction`/`VolumeRemoveCloudAction` implemented.
  4. ☑ CLI `sd volume add-cloud` / `remove-cloud` with custom-endpoint support.
  5. ☑ Query-system support for cloud paths (`Entry::try_from`, `DirectoryListingQuery`, `FileByPathQuery`).
  6. ☑ Indexer uses `VolumeBackend` — discovery via `read_dir`, content via ranged reads; **change detection skipped for cloud**.
- **Acceptance criteria:**
  - ☑ Add S3 bucket as a location.
  - ☐ **Files can be copied to/from the cloud volume.** ← gap (tracked by FILE-003).
  - ☑ Cloud volume indexes like any other location.
- **Completion:** 2 / 3 AC; all 6 impl-step blocks claim ☑ but file-ops acceptance is unchecked.
- **Next steps** (`.tasks/core/CLOUD-003-cloud-volume.md:76-80`):
  1. Test end-to-end with MinIO or real S3.
  2. Implement file copy for cloud volumes (FILE-003).
  3. OAuth for Google Drive / Dropbox / OneDrive.

---

### FILE-003 — Cloud Volume File Operations

- **Status:** **To Do** (note contradicting FILE-000 which lists it as "Done").
- **Assignee:** jamiepine
- **Priority:** High
- **Parent:** FILE-000
- **Related:** FILE-001, CLOUD-003, VOL-004
- **Whitepaper:** Section 4.4.6
- **Objective:** Add `CloudCopyStrategy` so the existing `FileCopyJob` works for cloud-local, local-cloud, and cloud-cloud transfers.
- **Implementation steps:**
  1. ☐ Create `CloudCopyStrategy` using `VolumeBackend` for I/O; streaming + progress + chunked.
  2. ☐ Update `CopyStrategyRouter` to detect `SdPath::Cloud` and route cross-combinations.
  3. ☐ Cloud-aware strategy selection (native copy on same provider; stream otherwise).
  4. ☐ Progress tracking, interruption handling, resume support where backend allows.
  5. ☐ Integrate with `CopyAction` + validation.
- **Acceptance criteria:**
  - ☐ Copy local → cloud.
  - ☐ Copy cloud → local.
  - ☐ Copy between two cloud volumes.
  - ☐ Accurate progress reporting.
  - ☐ Cancellation mid-operation.
  - ☐ Checksum verification.
- **Completion:** 0 / 6.
- **Technical notes** (`.tasks/core/FILE-003-cloud-volume-file-operations.md:74-79`): use `backend.read_range()` for chunks; rate-limit cloud calls; distinguish network vs. cloud errors; investigate OpenDAL native copy for same-backend cloud-to-cloud.

---

### VOL-004 — Cloud Volume Indexing with OpenDAL

- **Status:** Done (`last_updated: 2025-10-14`)
- **Assignee:** jamiepine
- **Priority:** High
- **Whitepaper:** Section 4.3.5
- **Parent:** VOL-000
- **Related:** CLOUD-003
- **Objective:** Wire `opendal` crate as the cloud backend and enable indexing of S3-compatible services.
- **Implementation steps:**
  1. ☑ Add `opendal` crate with S3/GCS/Azure feature flags.
  2. ☑ `CloudBackend` wrapping `opendal::Operator`, implementing `VolumeBackend`.
  3. ☑ `read`, `read_range`, `read_dir`, `metadata`, `write`, `delete`.
  4. ☑ `VolumeManager` integration; query-system supports remote paths; indexer uses `VolumeBackend`.
  5. ☑ CLI: `sd volume add-cloud` / `remove-cloud`; secure OS-keyring credential storage.
- **Acceptance criteria:** ☑ ☑ ☑ (all three met).
- **Next steps:** OAuth for consumer services, perf testing.

---

### SEC-005 — Secure Credential Vault

- **Status:** Done
- **Assignee:** jamiepine
- **Priority:** High
- **Parent:** SEC-000
- **Whitepaper:** Section 8
- **Objective:** Encrypted vault for API keys / secrets so libraries can connect to cloud accounts safely.
- **Implementation steps:**
  1. ☐ DB schema with encryption-at-rest. (checklist unchecked in file but status = Done — see §5 Open Questions)
  2. ☐ Add/update/delete credentials.
  3. ☐ OS keychain for master key.
  4. ☐ Integrate with cloud volume system.
- **Acceptance criteria:** all three unchecked in file, despite Done status. Implementation lives at `core/src/crypto/cloud_credentials.rs` and `core/src/crypto/key_manager.rs`.

---

### SEC-006 — Certificate Pinning

- **Status:** To Do
- **Priority:** Medium
- **Parent:** SEC-000
- **Whitepaper:** Section 8
- **Objective:** Pin certificates for all connections to third-party cloud providers to prevent MITM.
- **Implementation steps:**
  1. ☐ Integrate pinning library.
  2. ☐ Obtain public-key fingerprints for trusted providers.
  3. ☐ Enforce pinning in networking stack.
  4. ☐ Mechanism to update pinned certs.
- **Acceptance criteria:** all three unchecked.
- **Note:** Parent SEC-000 lists this as `SEC-004` — naming drift.

---

### Related / cross-cutting tasks

| Task | Status | Cloud relevance |
|---|---|---|
| FILE-004 (Rename & Folders) | Done | `VolumeBackend.create_directory()`; `CloudBackend` has **stub** implementation — folder creation not actually working on cloud yet (`.tasks/core/FILE-004-rename-and-folders.md:38`). |
| FILE-005 (Bidirectional Remote Copy / PULL) | Done | Discusses cross-cloud PULL as future optimisation: "Pull from cloud via remote device — Remote device proxies from cloud to local device … Future: Direct cloud pull optimization" (`.tasks/core/FILE-005-bidirectional-remote-copy.md:436-439`). |
| FSYNC-001 (Delete Strategy Pattern) | Done | Added `VolumeBackend.delete()`; `LocalDeleteStrategy` handles cloud paths via `OpenDAL delete/remove_all` — no separate `CloudDeleteStrategy` needed (`.tasks/core/FSYNC-001-delete-strategy-pattern.md:155-161`). |
| RED-000 (Redundancy Awareness) | — | Design decision: "Cloud volumes: No special treatment. Cloud volumes are treated the same as any other volume — you can lose access to a cloud provider, so a file only on S3 is still at risk." (`.tasks/core/RED-000-redundancy-awareness.md:276`). |
| LSYNC-022 (Sync Metrics) | — | Mentions future "Cost tracking: Estimate cloud egress costs for bandwidth usage" (`.tasks/core/LSYNC-022-sync-metrics-and-observability.md:513`). |
| INDEX-009 (Stale File Detection) | — | References `cloud_url_base` parameter threading through `read_directory` (`.tasks/core/INDEX-009-stale-file-detection.md:190`). |

---

## 5. Open Questions / Gaps

### Status contradictions

1. **FILE-003 status drift.** FILE-000 (epic) lists FILE-003 as "Done" (`.tasks/core/FILE-000-file-operations.md:19`). The FILE-003 file itself has `status: To Do` and all 6 acceptance criteria unchecked (`.tasks/core/FILE-003-cloud-volume-file-operations.md:4,49-54`). FILE-003 is the **critical dependency** for the CLOUD-003 acceptance criterion "Files can be copied to and from the cloud volume". **The parent epic misrepresents the state.**
2. **SEC-005 unchecked acceptance but "Done"** — all four implementation steps and all three acceptance criteria are unchecked (`.tasks/core/SEC-005-secure-credential-vault.md:16-27`), yet `status: Done`. The implementation clearly exists (`core/src/crypto/cloud_credentials.rs`); the task file just wasn't updated. Needs AC back-fill or the status should revert to "In Progress" per the CLAUDE.md rigor guidance.
3. **SEC-000 numbering.** SEC-000 lists certificate pinning as `SEC-004` (`.tasks/core/SEC-000-security-and-privacy.md:21`), but the actual task file is `SEC-006-certificate-pinning.md`. `SEC-004` is now `SEC-004-rbac-system.md`. The epic index is out of date.
4. **CLOUD-003 impl steps say ☑ but AC says ☐** for "Files can be copied to/from cloud". The 6 implementation blocks under CLOUD-003 are all marked done, but the 2nd acceptance criterion remains unchecked because it legitimately depends on FILE-003 which is not done. Internally consistent but confusing: implementation steps conflate "architecture ready" with "feature works end-to-end".

### Features in docs but not in any task

1. **iCloud Drive, pCloud, MEGA** — listed under "Consumer Cloud" in `docs/core/cloud-integration.mdx:86`. No task file, no implementation plan. iCloud Drive is conspicuous for an Apple-ecosystem file manager.
2. **SharePoint, Box, Nextcloud, Seafile, WebDAV** — listed under "Enterprise Storage" (`docs/core/cloud-integration.mdx:88`). No tasks.
3. **Alibaba OSS, Tencent COS, Huawei OBS, Baidu BOS** — listed under "Regional Services" (`docs/core/cloud-integration.mdx:90`). No tasks.
4. **FTP / SFTP / SMB** — mentioned in VOL-004 description (`.tasks/core/VOL-004-remote-volume-indexing-with-opendal.md:16`) but not in the supported/planned table or anywhere else.
5. **Cloud provider webhooks** for real-time change notifications — called out as a `<Tip>` in cloud-integration docs (`docs/core/cloud-integration.mdx:363-366`) but no task models it.
6. **Cost tracking / egress estimation** — mentioned only in LSYNC-022:513 as a future concept.
7. **Metadata cache** — "Directory listings and file information cache for 5 minutes" (`docs/core/cloud-integration.mdx:219`). Implementation location unspecified; no cache-invalidation strategy documented beyond "automatic on file modifications" which conflicts with "no change detection".
8. **Content cache / thumbnail cache** — described as intelligent, but no task defines eviction policy, size limits, or scope.

### TBD / decision-pending items

1. **Change detection against cloud metadata.** Explicitly declared as **not implemented** (`docs/core/cloud-integration.mdx:370`): cloud files are treated as new on every reindex. No task exists to fix it. Needs: a design decision on whether to use ETags / Last-Modified / version IDs and how to reconcile provider-specific semantics.
2. **Real-time file watcher for cloud.** "Cloud volumes do not support real-time file monitoring" (`docs/core/cloud-integration.mdx:372`). Webhooks are the obvious answer (provider-specific), but no task captures the work.
3. **OAuth token refresh.** "Token refresh is not yet implemented. You must re-authenticate manually when tokens expire" (`docs/core/cloud-integration.mdx:374`). Also blocks the Google/Dropbox/OneDrive integrations claimed as "implemented" in `docs/core/volumes.mdx:270-275`.
4. **CLOUD-001 / CLOUD-002** are pure-design stubs — technology selection (Kubernetes vs. Nomad vs. serverless), multi-tenant isolation model, provisioning lifecycle, cost model, and relay protocol design are all open questions.
5. **Cloud-to-cloud native copy** — FILE-003 step 3 acknowledges this as TBD: "investigate if OpenDAL supports native copy operations" (`.tasks/core/FILE-003-cloud-volume-file-operations.md:79`).
6. **Cross-cloud PULL optimisation.** FILE-005 notes the current model goes cloud → remote → local, "slow". Direct cloud pull is marked as future work (`.tasks/core/FILE-005-bidirectional-remote-copy.md:436-439`).
7. **`create_directory` on cloud** is a stub (`.tasks/core/FILE-004-rename-and-folders.md:38`). Object stores don't have real directories; the semantics need to be pinned down (marker files? prefix-only?).
8. **Certificate pinning list maintenance** — SEC-006 mentions "a mechanism for updating the pinned certificates" but doesn't specify who ships updates (Spacedrive release, remote config, etc.).
9. **Slug collision for cloud mount points** — docs say mount points get `-2`, `-3` suffixes on collision (`docs/core/addressing.mdx:440-447`). What happens when a user re-adds a cloud volume previously removed? Not specified.
10. **Library-level sync of cloud volume configuration.** If a user adds an S3 bucket to a library, does that config propagate to paired devices (so everyone sees the bucket)? The `cloud_identifier` field is on `Volume`, but `Volume` ownership is per-device. Unclear design intent — probably *not* shared (credentials are per-library, keyed by device's keyring).

### Contradictions between docs and tasks

1. **"Cloud credentials are stored in OS keyring"** (`docs/core/volumes.mdx:283`) vs. **"Cloud credentials are stored in the library database, but encrypted using library keys from KeyManager"** (`docs/core/key-manager.mdx:100`). Both are partially true: the encrypted blob lives in the library DB, the key lives in the keyring. The volumes doc phrasing is imprecise.
2. **"Supports 40+ cloud services via OpenDAL"** (`docs/core/volumes.mdx:75`) vs. actual coverage in tasks (only S3-family tested end-to-end, Google/Dropbox/OneDrive backend-wired without OAuth). The 40+ number is about OpenDAL's capabilities, not Spacedrive's shipped feature set.
3. **"Once indexed, you can search, browse thumbnails, and view metadata for cloud files even when offline"** (`docs/core/cloud-integration.mdx:36-40`) — this implies thumbnails are pre-generated and locally cached, but no task covers cloud-volume thumbnail generation at scale. Thumbnail sidecars are location-independent in principle (they key off content identity) but the flow of "fetch ranged bytes → generate thumbnail → cache as sidecar" for cloud files isn't spec'd.
4. **History doc says the team pivoted away from cloud** ("V2 pivots to premium extensions following the COSS model", `docs/overview/history.mdx:114,218`) — yet CLOUD-000/001/002 (Managed Cloud Service / Relay) remain active High-priority tasks in `.tasks/`. Either the "Cloud as a Peer" vision is exempt from the pivot because it's optional/paid-tier, or the tasks haven't been updated to reflect the strategic shift. The introduction doc is careful to call cloud services "optional" (`docs/overview/history.mdx:287`).

---

## 6. Priorities & Roadmap

The documentation does not publish an explicit versioned roadmap, but an MVP can be reconstructed from task status, acceptance criteria, and the "Next Steps" sections.

### Phase 1 — MVP (cloud-as-a-volume for S3)

**Target: S3-family users can mount, index, and operate on an S3 bucket as if it were a local disk.**

| Work | Task | Status |
|---|---|---|
| `VolumeBackend` trait + OpenDAL `CloudBackend` | VOL-004, CLOUD-003 | ☑ Done |
| Credential vault (per-library XChaCha20-Poly1305 + OS keyring) | SEC-005 | ☑ Done (AC unchecked) |
| Query system: `Entry`, `DirectoryListingQuery`, `FileByPathQuery` for `SdPath::Cloud` | CLOUD-003 | ☑ Done |
| Indexer: discovery + content phases use `VolumeBackend` | CLOUD-003 | ☑ Done |
| CLI `sd volume add-cloud` / `remove-cloud` | CLOUD-003 | ☑ Done |
| Delete operations on cloud volumes | FSYNC-001 | ☑ Done |
| **File copy to/from cloud volumes** | FILE-003 | ☐ **Blocking MVP** |
| End-to-end testing against real S3 / MinIO | CLOUD-003 next-steps #1 | ☐ |

MVP is ~85% done. **FILE-003 is the single remaining feature gap** for the S3 MVP story.

### Phase 2 — Consumer cloud (Google Drive, Dropbox, OneDrive)

| Work | Task | Status |
|---|---|---|
| OpenDAL backend wiring | VOL-004 | ☑ Done |
| **Native OAuth 2.0 with PKCE** | (no task yet — referenced in `docs/core/volumes.mdx:385`) | ☐ |
| **OAuth token refresh** | (no task — `docs/core/cloud-integration.mdx:374`) | ☐ |
| End-to-end testing | VOL-004 next-steps | ☐ |

**Blocked on OAuth.** No task file captures this work; this is the largest unbooked chunk in the cloud roadmap.

### Phase 3 — Enterprise & protocol-level

- **WebDAV, Nextcloud, SharePoint, Box** — no tasks.
- **FTP / SFTP / SMB** — mentioned in VOL-004 scope but not tracked.
- **Change detection, webhooks, incremental reindex** — no tasks.
- **Thumbnail generation pipeline for cloud volumes** — no task.
- **Metadata caching strategy (invalidation, eviction)** — no task.
- **Cost tracking / egress estimation** — referenced only in LSYNC-022.

### Phase 4 — Cloud as a Peer (Managed Cloud)

Entirely **design-stage**. No implementation tasks yet.

| Work | Task | Status |
|---|---|---|
| Infrastructure design (Kubernetes / multi-tenancy / provisioning / security) | CLOUD-001 | ☐ Design |
| Async relay server | CLOUD-002 | ☐ Design |
| Managed `sd-core` per user | CLOUD-000 | ☐ Epic |
| Certificate pinning (related) | SEC-006 | ☐ |

This is the most architecturally ambitious piece — and directly conflicts with the V1 post-mortem's "cloud focus was a mistake" conclusion. Whether this phase actually ships depends on the business-model strategy documented in `docs/overview/history.mdx:211-221`.

### Stated MVP (from cloud-integration docs)

The `docs/core/cloud-integration.mdx` doc doesn't explicitly brand anything as "MVP", but its code examples only show S3 configuration. The flagship user stories in the doc are:

- `VolumeAddCloudInput { service: S3, ... }` (`docs/core/cloud-integration.mdx:17-32`)
- `FileCopyAction` between `gdrive://` and `local://` (`docs/core/cloud-integration.mdx:172-187`)
- Cloud-to-cloud move between S3 and Google Drive (`docs/core/cloud-integration.mdx:189-206`)

The docs advertise cloud-to-cloud as a working feature but the task system shows FILE-003 unstarted — **this is a marketing-vs-reality gap** worth flagging to stakeholders.

---

## Sources

**Task files** (in `E:\spacedrive\.tasks\core\`):

- `CLOUD-000-cloud-as-a-peer.md`
- `CLOUD-001-design-cloud-core-infra.md`
- `CLOUD-002-relay-server.md`
- `CLOUD-003-cloud-volume.md`
- `VOL-000-volume-operations.md`
- `VOL-004-remote-volume-indexing-with-opendal.md`
- `FILE-000-file-operations.md`
- `FILE-003-cloud-volume-file-operations.md`
- `FILE-004-rename-and-folders.md` (cloud-relevant sections)
- `FILE-005-bidirectional-remote-copy.md` (cloud-relevant sections)
- `FSYNC-001-delete-strategy-pattern.md` (cloud-relevant sections)
- `SEC-000-security-and-privacy.md`
- `SEC-005-secure-credential-vault.md`
- `SEC-006-certificate-pinning.md`
- `RED-000-redundancy-awareness.md` (cloud-relevant sections)
- `LSYNC-022-sync-metrics-and-observability.md` (cloud-relevant sections)
- `INDEX-009-stale-file-detection.md` (cloud-relevant sections)

**Documentation files** (in `E:\spacedrive\docs\`):

- `core/cloud-integration.mdx` (primary doc, 381 lines)
- `core/volumes.mdx` (447 lines)
- `core/addressing.mdx` (456 lines)
- `core/data-model.mdx` (partial, `SdPath` + `Volume` sections)
- `core/key-manager.mdx` (394 lines)
- `core/architecture.mdx`
- `overview/whitepaper.mdx` (summary — full PDF at `whitepaper/spacedrive.pdf`)
- `overview/history.mdx` (293 lines — business-model context)
- `overview/philosophy.mdx` (cloud-relevant intro sections)
- `overview/introduction.mdx` (cloud-relevant sections)
- `overview/get-started.mdx` (example usage)
- `core/design/file-system-intelligence.md` (cloud policy/permissions mentions)
- `core/design/archive.md` (OAuth adapter historical reference)
- `core/task-tracking.mdx` (contains the CLOUD-003 task example)

**Not read in this pass:**

- The full PDF whitepaper at `whitepaper/spacedrive.pdf` — sections 5.1 / 5.2 / 5.3 / 4.3.5 / 4.4.6 / 8 are referenced from tasks; the markdown `whitepaper.mdx` is only a summary/invite doc. Deep reading of the PDF would resolve some of the open questions (especially the intent behind CLOUD-001 "Managed Cloud Core Infrastructure").
- Any source code under `core/src/volume/backend/`, `core/src/crypto/cloud_credentials.rs`, `core/src/ops/volumes/` — deferred to later investigations per the "no code" scope.
