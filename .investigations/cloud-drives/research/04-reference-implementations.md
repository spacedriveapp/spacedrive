# Cloud Drives — Reference Implementations Survey

Research date: 2026-04-18
Scope: multi-cloud drive handling from native desktop apps. Six products surveyed with emphasis on rclone (Go) and object_store (Rust) as the most directly applicable to Spacedrive.

---

## 1. Executive Summary

Spacedrive's cloud-drive subsystem shares the core problem set that rclone, Nextcloud Desktop, Syncthing, Cyberduck, Kopia and Apache Arrow's `object_store` have all solved in production: abstract heterogeneous backends behind one trait, authenticate with per-provider quirks, detect remote changes without re-scanning everything, reconcile against a local mirror, and keep the UI honest about sync state. rclone is the richest reference for backend breadth (60+ providers, a minimal `Fs`/`Object` interface, and a per-backend pacer); `object_store` is the tightest Rust-native design (async trait, conditional PUT/GET for OCC, vectored IO) and is the model Spacedrive should study line-by-line.

Top 5 lessons:

1. **Keep the core trait small; push capability through optional sub-traits or a `Features` bitmask.** (rclone `fs.Fs` + `fs.Features`; object_store `ObjectStore` + `ObjectStoreExt`).
2. **Treat "what can this backend do?" as first-class runtime data**, not compile-time assumptions. Hash algos, modtime precision, case sensitivity, duplicate-filename tolerance and server-side move all vary per provider.
3. **Change detection is the hard part.** Use provider-native delta APIs where they exist (Drive `changes.list`, OneDrive `delta`, Dropbox `list_folder/continue`) and fall back to scan+compare. A Syncthing-style `{index_id, max_sequence}` tuple is the right abstraction over the top.
4. **Per-backend pacer with jittered exponential backoff and per-error-code fatal/retry classification.** rclone's Drive backend distinguishes `rateLimitExceeded` (retry), `userRateLimitExceeded` (retry), `downloadQuotaExceeded` (fatal-if-opted-in) — this level of granularity is non-negotiable.
5. **File identity survives rename only if you use the backend's opaque ID.** Path-based identity breaks every time a user drags a folder. Store `(backend_id, file_id)` or `(backend_id, etag/version)` as the persistent identity in your local DB; path is a secondary index.

---

## 2. Product Reviews

### 2.1 rclone — the reference for multi-cloud backend abstraction

#### Overview
rclone is a Go CLI and library ("rsync for cloud storage") supporting 60+ storage systems including S3, Google Drive, OneDrive, Dropbox, Azure Blob, B2, SFTP, WebDAV, FTP, and more. MIT licensed, 56.7k stars, actively maintained (latest release v1.73.4). Linux/macOS/Windows/FreeBSD/Plan9. Includes a FUSE mount, an HTTP/JSON-RPC control API (`--rc`), and a C-ABI library (`librclone`).

#### Backend abstraction
rclone defines a single trait `fs.Fs` in [`fs/types.go`](https://github.com/rclone/rclone/blob/master/fs/types.go). It is intentionally minimal:

```go
type Fs interface {
    Info
    List(ctx, dir) (DirEntries, error)
    NewObject(ctx, remote) (Object, error)
    Put(ctx, in io.Reader, src ObjectInfo, options ...) (Object, error)
    Mkdir(ctx, dir) error
    Rmdir(ctx, dir) error
}
```

Optional capabilities are expressed two ways:

1. **Optional interfaces** that a backend may implement — `Purger`, `Copier`, `Mover`, `DirMover`, `ListRer`, `PutStreamer`, `Abouter`, `MergeDirser`, `ChangeNotifier`, `PublicLinker`, `IDer`, `MimeTyper`, `Metadataer`, `SetMetadataer`, etc. Dispatch is by Go type-assertion at call sites.
2. **`Features` struct** returned by `Fs.Features()` — a runtime bitmask/struct that describes capabilities: `DuplicateFiles`, `CaseInsensitive`, `ReadMimeType`, `WriteMimeType`, `CanHaveEmptyDirectories`, `ServerSideAcrossConfigs`, `BucketBased`, `ReadMetadata`, `WriteMetadata`, `UserMetadata`, etc. See [Google Drive's feature struct initializer](https://github.com/rclone/rclone/blob/master/backend/drive/drive.go#L600-L625).

A backend registers itself in `init()` by calling `fs.Register(&fs.RegInfo{Name:"drive", NewFs: NewFs, Options: [...]})`. Adding a new backend is a single package under `backend/<name>/` that implements `Fs` plus whichever optional interfaces make sense. The `cmd/serve/*` and `sync/*` packages work against the plain `Fs` trait and dynamically detect optional features.

**Verdict:** The minimal-core + optional-interfaces design is 10 years proven over 60 backends. Spacedrive should steal this wholesale.

#### Authentication
- **OAuth.** rclone ships `lib/oauthutil` with a full local-loopback flow: it starts an HTTP server on `127.0.0.1:53682`, opens the browser to the provider's authorize URL, receives the code on the loopback callback, exchanges it for a token, and writes the token JSON into the config file. For headless setups there's "manual mode" where the user copies a code out of their browser on another machine and pastes it into the CLI.
- **Client ID/Secret.** Each backend ships with a baked-in default client ID/secret. For Google Drive the secret is obfuscated via `obscure.MustReveal` of a constant. Users are encouraged to bring their own client ID to escape the shared rate limit. See `backend/drive/drive.go` constants `rcloneClientID` / `rcloneEncryptedClientSecret`.
- **Credential storage.** A single flat file `~/.config/rclone/rclone.conf` in INI format, optionally encrypted with `rclone config password`. Tokens live inline as JSON strings.
- **Token refresh.** Handled by `golang.org/x/oauth2` — rclone wraps the HTTP client with an `oauth2.Transport` that auto-refreshes using the refresh token when the access token expires.
- **Revocation.** If the refresh token is rejected, calls surface `oauth2.RetrieveError` and rclone asks the user to re-run `rclone config reconnect`. No proactive push.

- **Service accounts** (Google Drive/GCS) and **env-based IAM** (S3/GCP) supported as alternatives to interactive OAuth.

#### Change detection & sync engine
rclone does not maintain a persistent sync journal (except for `bisync`). Its default `sync`/`copy` operation:

1. Lists source + destination recursively (with `ListR` where supported).
2. Walks both listings in sorted order, comparing on `(name, size, modtime, hash)`.
3. Transfers files that differ. No local DB; every sync is full-scan.

`bisync` is the 2-way sync command and it does maintain state: a `listing` snapshot per side saved as sorted text files in the state directory, used to compute 3-way diffs (`prev local | cur local | cur remote`). See `cmd/bisync/`.

The `mount` command's VFS cache (`vfs/`) maintains an in-memory directory cache with configurable TTL and can subscribe to `ChangeNotifier` where backends implement it (Drive, OneDrive).

#### File identity & rename tracking
- rclone's default identity is the **path string**. Rename is detected only in limited cases.
- Backends with opaque IDs (Drive `FileId`, OneDrive `id`, B2 `fileId`, Box `id`) expose the `IDer` interface. `sync` doesn't use these by default; `bisync` and `copy --track-renames` can use hash+size to detect renames and issue a server-side move.
- `--track-renames-strategy` takes `hash,modtime,leaf` — pick what matters per provider.

#### Streaming / large file handling
- Per-backend `chunk_size` / `upload_cutoff` options. Google Drive defaults: 8 MiB chunk, 8 MiB cutoff. S3: 5 MiB min chunk (S3's minimum), configurable up to 5 GiB. B2: 96 MiB chunks.
- Resumable uploads: Drive uses Google's resumable upload protocol; S3 uses multipart upload with part-upload retries; OneDrive uses upload sessions.
- Multi-threaded downloads: `--multi-thread-streams=N` splits a single file download into N parallel range requests.
- `--streaming-upload-cutoff` for stdin.

#### Error handling & retries
- Every backend initializes its own `fs.Pacer` (`lib/pacer`) with backend-specific strategy. Drive uses `pacer.NewGoogleDrive(MinSleep=100ms, Burst=100)` — a token bucket that also reacts to 429/5xx by exponential backoff.
- `shouldRetry(err)` is per-backend. [Drive's version](https://github.com/rclone/rclone/blob/master/backend/drive/drive.go#L830-L870):
  - 5xx → retry
  - `rateLimitExceeded` / `userRateLimitExceeded` → retry (unless `stop_on_upload_limit` and message is "User rate limit exceeded." → fatal)
  - `downloadQuotaExceeded` → fatal if `stop_on_download_limit`, else retry
  - `quotaExceeded` / `storageQuotaExceeded` → fatal if `stop_on_upload_limit`
  - `teamDriveFileLimitExceeded` → fatal
- Global `--retries` (default 3) wraps each transfer; `--low-level-retries` (default 10) wraps each HTTP request.
- Transient network errors caught via `fserrors.ShouldRetry(err)` which checks for `net.Error.Temporary()` and specific errno values.

#### UI/UX
- rclone is CLI-first. The interactive `rclone config` walks through a state-machine prompt: pick backend → fill options → OAuth flow → test connection → save.
- `rclone authorize` is a helper that runs the OAuth loopback on a browser-equipped machine and spits out a token JSON for the user to paste into a headless install.
- Errors surface as plain log lines; the `rcd` daemon has a `core/stats` endpoint returning JSON with `transferring`, `checking`, `errors`, `lastError`, `eta` (see `rclone rc` docs). A React web GUI consumes this.
- No built-in re-auth UI — users must re-run `rclone config reconnect <remote>` manually.

#### Key lessons for Spacedrive
1. **`Fs`+`Features` is the gold-standard trait pattern.** Minimal required methods, capability detection via runtime struct, optional sub-traits for server-side copy/move/link-sharing/change-notify.
2. **Per-backend pacer is essential.** Different providers have different shapes of rate limit. Spacedrive cannot have one global retry policy; each backend must own its retry/classify logic.
3. **Ship default OAuth client credentials but let users BYO.** Shared credentials hit shared rate limits. Drive has an entire docs section on "making your own client ID."
4. **Restricted-character encoding is a real problem.** rclone's `lib/encoder` maps invalid characters to Unicode fullwidth equivalents per-backend and reverses on download. Spacedrive will hit this the first time a user has `file:1.txt` on macOS and tries to upload to OneDrive.
5. **Don't try to abstract away Google Docs / shortcuts / etc.** rclone exposes them via `--drive-export-formats`, `--drive-skip-shortcuts`, `--drive-skip-gdocs`. These are provider-specific weirdnesses that leak through and the right call is to expose them, not hide them.

#### Tech stack
- HTTP: `net/http` + custom transport in `fs/fshttp`
- OAuth: `golang.org/x/oauth2`
- Hashing: `crypto/md5`, `crypto/sha1`, `crypto/sha256`, plus per-backend custom hashers (Dropbox `dropbox`, Jottacloud `jottacloud`, OneDrive `quickxor`)
- Pacing: custom `lib/pacer` (token bucket + exponential backoff with jitter)
- Config: INI via a custom `fs/config/configfile` parser
- Concurrency: native goroutines + `errgroup`

---

### 2.2 Apache Arrow `object_store` (Rust) — the Rust-native design reference

#### Overview
A focused, high-performance, async Rust crate providing a uniform API over S3, Azure Blob, GCS, local filesystem, HTTP/WebDAV, and in-memory stores. Apache-2.0 licensed, originally InfluxData, donated to Apache Arrow. Repo: https://github.com/apache/arrow-rs-object-store (moved out of arrow-rs in 2025). Current version: 0.13.2 (docs.rs), 239 stars on the split-out repo, used in DataFusion, delta-rs, influxdb-iox, crates.io.

#### Backend abstraction
Single core trait: `ObjectStore` in [`src/lib.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/lib.rs). As of 0.13.0, deliberately minimal:

```rust
#[async_trait]
pub trait ObjectStore: Display + Send + Sync + Debug + 'static {
    async fn put_opts(&self, location: &Path, payload: PutPayload, opts: PutOptions) -> Result<PutResult>;
    async fn put_multipart_opts(&self, location: &Path, opts: PutMultipartOptions) -> Result<Box<dyn MultipartUpload>>;
    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult>;
    async fn get_ranges(&self, location: &Path, ranges: &[Range<u64>]) -> Result<Vec<Bytes>>;
    fn delete_stream(&self, locations: BoxStream<'static, Result<Path>>) -> BoxStream<'static, Result<Path>>;
    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>>;
    fn list_with_offset(&self, prefix: Option<&Path>, offset: &Path) -> BoxStream<'static, Result<ObjectMeta>>;
    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult>;
    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()>;
    async fn rename_opts(&self, from: &Path, to: &Path, options: RenameOptions) -> Result<()>;
}
```

Convenience methods (`put`, `get`, `get_range`, `head`, `delete`, `copy`, `rename`, `copy_if_not_exists`, `rename_if_not_exists`) moved to `ObjectStoreExt` — a blanket-impl trait users call but implementors don't touch. This was deliberate: default implementations are "convenient for users, error-prone for implementors" (per the `ObjectStore` doc comment).

Adding a backend = one file under `src/<name>/` with a builder (`AmazonS3Builder`, `GoogleCloudStorageBuilder`, `MicrosoftAzureBuilder`) plus `impl ObjectStore for MyStore`. URL parsing is pluggable via `parse_url(&Url)` → `(Arc<dyn ObjectStore>, Path)`, so callers write:

```rust
let url = Url::parse("s3://bucket/path").unwrap();
let (store, path) = parse_url(&url)?;
```

#### Authentication
Each backend ships a `CredentialProvider` trait — a single async method returning the current credentials. Implementations include:
- `StaticCredentialProvider<T>` for static creds.
- AWS: `InstanceCredentialProvider`, `WebIdentityProvider`, env-based builder (`AmazonS3Builder::from_env()`), SSO/IMDS, `AWS_CONTAINER_CREDENTIALS_*`.
- Azure: `ImdsManagedIdentityProvider`, client-secret, shared-key, SAS token, workload identity, Azure CLI credential cache.
- GCP: service-account JSON, `GOOGLE_APPLICATION_CREDENTIALS`, metadata server.
- No OAuth interactive flow — the crate targets server/backend workloads, not desktop users. This is a gap Spacedrive will have to fill.

Token refresh is cached inside each provider (a `Mutex<Option<(Token, Instant)>>` refreshing on expiry). Thread-safe. Tokens are held in memory only; persistence is the caller's responsibility.

#### Change detection & sync engine
Not a sync engine — `object_store` is an object-store SDK. No listing journal, no delta API, no local DB. Change detection is delegated to the caller via:
- **ETags on `ObjectMeta`** (field `e_tag: Option<String>`)
- **Version on `ObjectMeta`** (field `version: Option<String>`)
- **Conditional GET** via `GetOptions::with_if_none_match(etag)` → returns `Error::NotModified`
- **Conditional PUT** via `PutMode::Update(UpdateVersion { e_tag, version })` → returns `Error::Precondition` on conflict

This is enough to build a cache or an OCC-style transaction log on top, and the docs explicitly demonstrate a cache pattern and an Iceberg/Delta Lake OCC commit loop.

#### File identity & rename tracking
- Paths only. `Path` is a typed wrapper enforcing object-store naming rules (UTF-8, no leading/trailing `/`, normalized).
- No native rename tracking. `rename()` = `copy` + `delete`.
- For versioned buckets, `ObjectMeta.version` provides cross-rename stable identity if the backend supports it.

#### Streaming / large file handling
- `put` for small payloads (single atomic PUT).
- `put_multipart` for large: returns `Box<dyn MultipartUpload>` with `put_part(PutPayload) -> UploadPart` futures and `complete()`. Parts uploaded in parallel via `WriteMultipart::new(upload).write(...)`.
- `get_ranges(&[Range])` coalesces adjacent ranges into a single request (threshold `OBJECT_STORE_COALESCE_DEFAULT` = 1 MiB by default).
- `PutPayload` is a `Vec<Bytes>` — non-contiguous, allows bump-free chunked writes.

#### Error handling & retries
- `RetryConfig` struct with `max_retries`, `retry_timeout`, `backoff: BackoffConfig { init_backoff, max_backoff, base }`. Default: decorrelated-jitter exponential backoff.
- Applied at the HTTP client layer via `client/retry.rs`.
- Error enum (`object_store::Error`) has specific variants: `NotFound`, `Precondition`, `NotModified`, `AlreadyExists`, `NotSupported`, `NotImplemented{operation, implementer}`, `Generic{store, source}`. Retry classification lives in the retry client and treats 429, 503, 504 and network errors as retryable.
- `LimitStore` wrapper caps concurrent requests to a configured semaphore.
- `ThrottleConfig` wrapper simulates latency/bandwidth for testing.

#### UI/UX
None — this is a library. The UX burden lives at the caller. But the error enum's specificity (`Precondition`, `NotModified`, `NotSupported { operation, implementer }`) makes it easy to translate to a UI.

#### Key lessons for Spacedrive
1. **This trait signature is almost exactly what Spacedrive needs**, extended with cloud-drive specifics (OAuth, change tokens, metadata).
2. **Deliberate minimalism beats aspirational completeness.** 0.13.0 explicitly ripped out default implementations because they drifted. Spacedrive should require every backend to implement every method, and put convenience on an extension trait.
3. **Builder pattern per backend** (`AmazonS3Builder`, `GoogleCloudStorageBuilder`) with `from_env()`, `with_credentials(...)`, `with_bucket_name(...)`, `build() -> Result<Self>` — this is idiomatic Rust and scales.
4. **URL → store dispatch is a winning UX.** `parse_url("s3://bucket/path")` → `(store, path)` means Spacedrive can take a single URI and route it.
5. **Conditional PUT/GET primitives should exist in Spacedrive's trait.** They're the foundation of any cache or OCC layer on top.

**Go-read files for Spacedrive implementer:**
- `src/lib.rs` — the trait definitions and crate-level docs. https://github.com/apache/arrow-rs-object-store/blob/main/src/lib.rs
- `src/aws/mod.rs` — builder pattern for a full cloud backend
- `src/aws/credential.rs` — chained credential providers
- `src/client/retry.rs` — retry/backoff implementation
- `src/multipart.rs` — multipart upload abstraction
- `src/list.rs` — paginated listing
- `src/path/mod.rs` — typed `Path` and normalization rules

#### Tech stack
- HTTP: `reqwest` with configurable `HttpConnector`
- Async: `tokio`, `async-trait`, `futures`
- TLS: `rustls` (with `rustls-native-certs` or `webpki-roots`)
- XML/JSON: `quick-xml`, `serde_json`, `serde`
- Hashing/crypto: `ring`, `md-5`, `base64`
- URL parsing: `url`
- Error: `thiserror`

---

### 2.3 Nextcloud Desktop Client — mature sync + VFS

#### Overview
C++/Qt 6 desktop client for Nextcloud (and ownCloud, its ancestor). GPL-2.0+, 3.7k stars. Windows/macOS/Linux. Fork of ownCloud client circa 2016. Single-protocol (WebDAV + Nextcloud extensions) so it isn't a multi-backend system, but its sync engine is one of the most battle-tested open implementations.

#### Backend abstraction
No abstraction — all calls go through a `QNetworkAccessManager`-based `AbstractNetworkJob` hierarchy (`src/libsync/networkjobs.{h,cpp}`, plus `propagatedownload.cpp`, `propagateupload*.cpp`). The "abstraction" is at the VFS layer: `Vfs` is an abstract class with `Vfs::Mode` = `Off | WithSuffix | WindowsCfApi | MacOsFileProvider` and per-OS plugins under `src/libsync/vfs/{cfapi, suffix, mac}/`.

#### Authentication
- OAuth2 with PKCE via `src/libsync/creds/` (`HttpCredentialsGui`, `WebFlowCredentials`).
- Legacy basic-auth also supported.
- Credential storage: **OS keychain** via `qt5keychain` — Windows Credential Manager, macOS Keychain, Linux Secret Service / KWallet. Fallback is a config file with obfuscation.
- Token refresh: handled per-request; on 401 the `CredentialsJob` re-runs the refresh flow.
- Re-auth UI: a system-tray notification + modal when refresh fails.

#### Change detection & sync engine
The `SyncEngine` in `src/libsync/syncengine.{h,cpp}` is the orchestrator. Key concepts:

1. **Discovery phase** (`discovery.cpp`, `discoveryphase.cpp`): walks the remote via WebDAV `PROPFIND` with Nextcloud's `oc-etag` property. Etags propagate up from files to parent directories — **if a directory's etag is unchanged, the entire subtree is skipped**. This is the secret weapon that makes Nextcloud's sync fast on large trees.
2. **Local discovery** has two styles (`LocalDiscoveryStyle` enum): `FilesystemOnly` (full scan) vs `DatabaseAndFilesystem` (only scan directories known to have been touched, based on `_localDiscoveryPaths` populated from OS file-watchers).
3. **Reconciliation**: the discovery phase emits `SyncFileItem` records with `Instruction::NEW`/`UPDATE_METADATA`/`SYNC`/`RENAME`/`REMOVE`/`CONFLICT`/`IGNORE`.
4. **Propagation** (`owncloudpropagator.cpp`, `bulkpropagatorjob.cpp`): executes the items.
5. **Journal** (`SyncJournalDb`, SQLite): persists per-file `(path, inode, modtime, size, etag, fileid, content_checksum, remote_perm, ...)` rows in table `metadata`. Also `conflicts`, `downloadinfo`, `uploadinfo`, `errors_blacklist`.

#### File identity & rename tracking
- Nextcloud server assigns a stable `oc-fileid` to every file. The journal stores `(path, fileid)`.
- On rename, `oc-fileid` is unchanged → the client detects the move and issues a server-side `MOVE` WebDAV verb.
- Local file identity uses the inode on Linux/macOS, the Windows file-index number on NTFS (unstable across reboots — a known sharp edge; see the `FileSystem::getInode` comment).
- The `DiscoveryPhase` does rename detection by building a map of `fileid -> local_path` on both sides and matching.

#### Streaming / large file handling
- **Chunked upload v2**: splits files into parts, uploads them to a server-side "upload folder", then a final `MOVE` assembles them. Handles resume on reconnect via the upload folder's persisted state.
- Default chunk size: 10 MB, configurable. Nextcloud's v2 chunking can upload parts in parallel.
- Streaming downloads go straight to a tempfile on disk, then atomic rename into place.
- **VFS (virtual files)**: on Windows (CfAPI) and macOS (File Provider), files can be "online-only" placeholders that hydrate on access. Journal tracks pin-state (`VfsItemAvailability`).

#### Error handling & retries
- `QNetworkReply` errors get classified; transient errors go to a retry queue.
- `errors_blacklist` in the journal tracks files that repeatedly fail; they're excluded from the next N syncs with exponential backoff.
- Rate limit: Nextcloud doesn't really rate-limit (it's self-hosted). Mostly surfaces as 5xx or timeout.
- Quota: server returns 507; client stops transfers and surfaces via `slotInsufficientRemoteStorage` signal.
- `bandwidthmanager.cpp` enforces user-configured upload/download rate limits.

#### UI/UX
- Single "Add Account" wizard launching the server-discovery + OAuth flow.
- Per-folder sync status in the system tray: hourglass, checkmark, warning overlay on file icons via a shell extension (Windows) / Finder sync extension (macOS) / Nautilus plugin (Linux).
- `ClientStatusReporting` subsystem aggregates per-error-type counts for telemetry/UI.
- Conflict files are renamed with `" (conflicted copy <user> <date>).<ext>"` suffix — never silently overwritten.

#### Key lessons for Spacedrive
1. **Etag propagation up the directory tree is the single most impactful sync optimization.** A `PROPFIND` on one folder tells you nothing or everything below it changed. For cloud drives with a similar capability (OneDrive's `eTag` on folders, Drive's `version` on parents), use it.
2. **Hybrid local discovery** (filesystem-watcher-driven path list fed into the DB+FS scan) is the right model. Don't full-scan locally every sync, but don't trust the watcher alone either.
3. **Persistent journal with `(local_id, remote_id, etag, modtime, size, content_hash)` is the minimum viable state schema.** Treat all six as keys; any mismatch triggers a specific instruction.
4. **Conflict files with a documented naming convention** is better than a dialog. Users learn it once.
5. **Per-file error blacklist with exponential backoff** prevents one malformed file from blocking every subsequent sync.

#### Tech stack
- HTTP: Qt `QNetworkAccessManager`
- XML/WebDAV: Qt XML + custom parser
- OAuth: custom PKCE flow with embedded `QWebEngineView`
- Storage: SQLite via `QSqlDatabase`
- Credential storage: `qt5keychain`
- VFS: Win32 Cloud Files API, macOS File Provider framework, extended attributes on Linux

---

### 2.4 Syncthing — block-level P2P sync

#### Overview
Go P2P file sync. MPL-2.0. Not a cloud-drive client, but its change-detection and block-exchange algorithms are the reference implementation for anyone building sync.

#### Backend abstraction
N/A — Syncthing peers are all Syncthing. But within a device, the `lib/fs` package abstracts over the local filesystem with a `Filesystem` interface that has implementations for basic, fake (test), case-insensitive-wrapper, and error-injecting.

#### Authentication
- Device IDs = SHA-256 hash of a self-signed TLS certificate's DER-encoded public key, base32-encoded, formatted as `MFZWI3D-BONSGYC-YLTMRWG-C43ENR5-QXGZDMM-FZWI3DP-BONSGYY-LTMRWAD` (with Luhn check digits per 7-char group).
- On connect: TLS 1.3 handshake, both sides compute peer device ID from presented cert, verify against their known-device list, drop connection if not recognized.
- No OAuth, no passwords, no central auth. Pure PKI. See [`docs.syncthing.net/dev/device-ids.html`](https://docs.syncthing.net/dev/device-ids.html).

#### Change detection & sync engine
This is where Syncthing shines. The **Block Exchange Protocol v1** (BEP v1, [spec](https://docs.syncthing.net/specs/bep-v1.html)) defines:

1. **Files are split into blocks** of 128 KiB → 16 MiB, power-of-two, chosen so a file has < 2000 blocks. Each block has a SHA-256 hash. File identity = name + version vector; file contents = ordered list of `BlockInfo{offset, size, hash}`.
2. **Version vector per file**: a map `{device_short_id -> counter}`. On any local edit, the local device's counter increments. Version vectors determine which side is newer (dominance relation) or whether it's a conflict.
3. **Sequence number per device**: every local DB update ticks a monotonic counter on the device. The file's `sequence` is that counter at edit time.
4. **Delta index exchange**: peers send `ClusterConfig` with `{index_id, max_sequence}` of what they have for each other's folders. The receiver only sends files whose `sequence > max_sequence`. The `index_id` is a 64-bit random folder identifier; if it changes (repo reset), full re-index.
5. **Scanner** (`lib/scanner/walk.go`): walks the folder, hashes each block, diffs against the DB. Uses a **weak hash cache** (a xxhash rolling hash) keyed by `(path, modtime, size)` to skip re-hashing unchanged files.

The local DB is LevelDB (now BoltDB in v2), keyed by `folder-id / device-id / sequence-number`. Each FileInfo stores the full block list.

#### File identity & rename tracking
- Identity = `(folder_id, name)` where `name` is the UTF-8 NFC-normalized path.
- Rename is not first-class — a rename looks like delete+create in BEP. However, if both sides see a delete with `blocks matching an existing create`, they **reuse the blocks** and never transfer them.
- Block-level dedup means moves are free even without explicit rename tracking.

#### Streaming / large file handling
- Everything is block-streamed. A "download" is a sequence of `Request{folder, name, offset, size, hash}` messages, the peer responds with `Response{id, data}`.
- Temp files are assembled from blocks; atomic rename on completion.
- `DownloadProgress` messages let other peers know which blocks a slow downloader has so they can request from it.

#### Error handling & retries
- Peer connection errors → reconnect with exponential backoff (capped at 60 min).
- Per-block error: request again from a different peer if available.
- Rate limits: none intrinsic (it's P2P), but `limits` bandwidth throttling per device.
- Disk full: surfaces as scan error and folder paused.

#### UI/UX
- Web GUI on `localhost:8384`, served by Syncthing itself. Shows per-folder scan status, per-file sync progress, peer list.
- Device pairing: user copies the 52-char device ID from one device to the other. QR codes help on mobile.
- No system-tray on most platforms natively — third-party wrappers (SyncTrayzor on Windows, syncthing-macos, etc.) provide tray UX.

#### Key lessons for Spacedrive
1. **Block-level hashing with a weak-hash cache keyed on `(path, modtime, size)` is the scanner primitive** every sync tool eventually needs. Saves 90%+ of hash work.
2. **Version vectors are strictly better than timestamps for deciding "newer".** They survive clock skew, identify true conflicts, and compose across any number of peers.
3. **`{index_id, max_sequence}` tuple for delta sync** is a beautiful abstraction: an opaque epoch (to handle resets) plus a monotonic counter (for incremental update). This is the right shape for Spacedrive's remote-change-token abstraction regardless of provider.
4. **Content-addressable blocks enable free moves and partial-file dedup** even without a rename API. If Spacedrive ever wants cross-provider move-without-re-upload this is the mechanism.
5. **Device IDs from cert hashes** is elegant but inapplicable here; Spacedrive auths to a cloud provider, not a peer.

#### Tech stack
- Crypto: `crypto/tls`, `crypto/sha256`, `golang.org/x/crypto/chacha20poly1305`
- DB: `go.etcd.io/bbolt` (BoltDB) or LevelDB
- Protocol: Protocol Buffers (`google.golang.org/protobuf`)
- Compression: LZ4
- Weak hash: xxhash

---

### 2.5 Cyberduck — multi-cloud desktop file transfer

#### Overview
Java (core) + Cocoa/Swift (macOS shell) + C#/.NET (Windows shell). GPL-3.0, 4.4k stars. Not a sync tool — it's a cloud file browser/transfer client (like Transmit or Forklift) with a tree UI and drag-drop transfers. Mountain Duck (commercial sibling, closed-source) adds FUSE-style mounting on top of the same core. Supports FTP/SFTP/WebDAV/S3/Azure Blob/B2/Dropbox/Google Drive/OneDrive/Box/Google Storage/OpenStack Swift/Nextcloud/ownCloud/iRODS/DRACOON/StoreGate/Brick/SMB/Manta and custom "profiles."

#### Backend abstraction
The `Protocol` interface (`core/src/main/java/ch/cyberduck/core/Protocol.java`) describes connection metadata (name, scheme, default port, OAuth URLs, regions, properties). The `FeatureFactory` mixin returns `<T> T getFeature(Class<T>)` so callers ask `protocol.getFeature(Read.class)`, `protocol.getFeature(Write.class)`, `protocol.getFeature(Move.class)`, `Copy`, `Delete`, `Directory`, `Find`, `AttributesFinder`, `Timestamp`, `Touch`, `Upload`, `Download`, `Share`, `Versioning`, `Quota`, `Location`, `AclPermission`, etc. Each backend package (`s3/`, `dropbox/`, `onedrive/`, `googledrive/`, `box/`, `webdav/`, `ftp/`, `sftp/`, ...) is a full Maven module implementing the features it supports.

This is essentially the same pattern as rclone — minimal core + optional capability interfaces — but more granular (each capability is a dedicated Java interface rather than a single `Features` struct).

Adding a backend = new Maven module with `Protocol` implementation + `Feature*` implementations. Registered via service loader (`META-INF/services`).

#### Authentication
- OAuth2 with per-backend defaults for client ID, auth URL, token URL, redirect URI, scopes, PKCE. All configured on the `Protocol` instance.
- Interactive flow uses the OS default browser + a local HTTP listener for the callback (oauth module at `oauth/`).
- Credential storage: OS keychain. macOS Keychain on macOS, Credential Manager on Windows (via `windows/Cyberduck/src/Ch.Cyberduck.Core/Local/` bindings), Secret Service on Linux.
- Token refresh: automatic via the `OAuth2RequestInterceptor` (Apache HttpClient interceptor) — on 401, try refresh, retry once; on second 401, surface reauth dialog.
- Multi-account: each bookmark is an independent account. Cyberduck's top-level concept is "bookmark" (≈ rclone's "remote"), which bundles (protocol, hostname, credentials, default path, nickname, icon).

#### Change detection & sync engine
Cyberduck isn't a sync tool — it's a transfer tool. For its sync feature it does a one-shot listing of source + dest and transfers the difference. No persistent DB. Mountain Duck adds on-demand fetch via FUSE.

#### File identity & rename tracking
Path-based. Backends that expose a file ID (Google Drive, OneDrive, Box) store it in `Path.attributes().getFileId()` but it's used mostly for API calls, not for identity tracking.

#### Streaming / large file handling
- Per-protocol segmented uploads with configurable chunk size.
- Multipart for S3/B2/Azure; upload sessions for OneDrive; resumable for Drive.
- Parallel transfers via a transfer queue (`ch.cyberduck.core.transfer`).

#### Error handling & retries
- Per-backend `DefaultRetryCallable` with configurable max retries (default 1). Classifies HTTP 4xx vs 5xx vs network.
- Rate limits handled per-provider: S3 uses Apache HttpClient's `ServiceUnavailableRetryStrategy` with exponential backoff + jitter.

#### UI/UX
- Single Cocoa/WPF tree view showing remote contents.
- "Open Connection" dialog picks a protocol → fills host/user/path → OAuth loopback for cloud providers.
- "Bookmarks" panel lists saved accounts with per-provider icons. Double-click to connect.
- Transfer queue panel shows in-progress and queued transfers with progress bars.
- Re-auth: a modal dialog with the OAuth URL, user clicks "Continue" → browser opens → callback fires.

#### Key lessons for Spacedrive
1. **"Account" (Cyberduck: "Bookmark") as a first-class concept above "volume"/"remote" is the right UX.** A Google Drive account can expose multiple drives (My Drive, Shared Drives, "Shared with Me"). The user's mental model is account → drive → folder, not "configure a remote per drive."
2. **Per-capability feature interfaces (`Read`, `Write`, `Move`, `Copy`, `Share`, `Versioning`) are more discoverable than a flat `Features` struct.** Each capability has a typed interface with typed inputs/outputs. Cyberduck's `protocol.getFeature(Move.class)` returning `null` for backends that don't support server-side move is cleaner than a boolean on a struct.
3. **"Connection profiles" as shippable `.cyberduckprofile` XML files** let third parties add custom S3-compatible backends without recompiling. rclone has no equivalent. Spacedrive could ship builtin profiles + let users drop in custom ones.
4. **Rich metadata on `Protocol`** (default port, schemes, OAuth URLs, regions, placeholder strings like "Enter your bucket name") drives the "Add Account" UI generically. One dialog, N providers.

#### Tech stack
- HTTP: Apache HttpClient 4/5
- OAuth: scribejava-core (abstraction for OAuth 1/2 flows)
- Crypto: BouncyCastle
- Keychain: JNA bindings to native APIs
- S3: AWS SDK for Java
- OneDrive: `nuxeo-onedrive-client`
- Google: `google-api-services-drive`
- Dropbox: `dropbox-core-sdk`
- DI: Spring Framework

---

### 2.6 Kopia — content-addressable backup to multi-cloud

#### Overview
Go, Apache-2.0, 13k stars. Backup tool (not sync) that writes **encrypted, deduplicated, compressed snapshots** to S3/GCS/Azure/B2/WebDAV/SFTP/local/Rclone-wrapped-anything. CLI and Electron GUI.

#### Backend abstraction
The `Storage` interface in `repo/blob/storage.go` is the smallest of any of the tools surveyed:

```go
type Storage interface {
    GetBlob(ctx, id BlobID, offset, length int64, output OutputBuffer) error
    GetMetadata(ctx, id BlobID) (Metadata, error)
    PutBlob(ctx, id BlobID, data Bytes, opts PutOptions) error
    DeleteBlob(ctx, id BlobID) error
    ListBlobs(ctx, prefix BlobID, callback func(Metadata) error) error
    Close(ctx) error
    ...
}
```

That's essentially it. Kopia builds content-addressable block storage → content-addressable object storage → label-addressable manifest storage in layers on top of this primitive.

Adding a backend = one package under `repo/blob/<name>/`. There are ~12 backends. Kopia also ships a "rclone" backend that shells out to an rclone process — transitively unlocking everything rclone supports.

#### Authentication
Each backend accepts opaque `Options` struct (e.g. `s3.Options{Endpoint, AccessKeyID, SecretAccessKey, SessionToken, Region, BucketName, ...}`). Credentials typically come from environment, AWS profile, GCP ADC, Azure CLI. No interactive OAuth; Kopia is a backup daemon, not a desktop client. Tokens cached in memory by the SDK.

#### Change detection & sync engine
Kopia's **Content-Addressable Block Storage (CABS)** layer ([architecture docs](https://kopia.io/docs/advanced/architecture/)):

1. On backup, files are split into blocks (typically ≤ 20 MB, using a rolling-hash chunker like Rabin fingerprinting for content-defined chunks).
2. Each block is SHA-256 or BLAKE2S hashed → `Block ID` (e.g. `6a9fc3a464a79360269e20b88cef629a`).
3. Identical blocks produce identical IDs → natural dedup.
4. Blocks are encrypted (AES256-GCM or CHACHA20-POLY1305) then **packed into 20-40 MB "Pack" blobs** with random names (`pb4cf8ca...`, `q7a9939...`, `xn0_20db79...`) uploaded to BLOB storage.
5. Indices map `BlockID -> (packFileName, offset, length)`. Indices are themselves BLOB objects with `x` prefix.

On restore/check, Kopia fetches indices, resolves block IDs to packs, fetches ranges.

**Change detection**: Kopia snapshots are immutable. "Change detection" is just "what's new since last snapshot?" — determined by walking the local tree and hashing files, comparing block IDs against the repository index. Files with unchanged `(inode, size, mtime)` are skipped (cache of last-hash).

#### File identity & rename tracking
- Inside a snapshot, files are addressed by Object ID (a hash of their content or their directory-listing hash for folders). See Kopia's CAOS layer.
- Across snapshots, moves/renames are **free** because block dedup means a renamed file shares all blocks with its old self — only the parent directory listing changes.
- Directory listings are themselves objects (prefix `k`), recursively.

#### Streaming / large file handling
- Large files: Rabin-chunked, blocks individually hashed, packed into shared pack blobs. The `x` prefix indicates indirect JSON that points at ordered block list:
  ```json
  {"stream":"kopia:indirect","entries":[
    {"l":2617867,"o":"e510796ba6ffd15649ea400b67ef6159"},
    {"s":2617867,"l":2751278,"o":"5c090744e0cad69d0d1aecd4ffd69f69"},
    ...
  ]}
  ```
- Multipart uploads to S3/GCS/Azure for large pack blobs.

#### Error handling & retries
- Backend-specific retry wrappers. S3 uses the AWS SDK's built-in retryer. Kopia also has a higher-level retry loop in `repo/blob/retrying/` that retries entire `PutBlob`/`GetBlob` calls on transient errors.
- "Compaction" / "maintenance" is a separate background process that repacks, re-indexes, and garbage-collects unreferenced blocks.

#### UI/UX
- Electron/web-served GUI showing snapshot list, sources, policies. Connect to a repository with a URL + password.
- "Add repository" flow = pick provider → enter credentials → enter repo password (encrypts data) → done.

#### Key lessons for Spacedrive
1. **Content-addressable storage is the right primitive for deduplication and cross-provider move-without-transfer.** If Spacedrive wants "copy from Dropbox to S3" to be a metadata-only operation when contents already exist, this is how.
2. **Pack small blocks into larger blobs** before uploading. Cloud providers charge per-request and have minimum-part-size rules. Kopia's 20-40 MB packs amortize. (Less relevant if Spacedrive is mirroring the user's existing cloud layout rather than creating a CAS repo.)
3. **Prefix-based blob naming encodes object type** at the storage layer: `p` = data pack, `q` = metadata pack, `x` = index, `s` = snapshot manifest. Makes listing-filtering cheap.
4. **Separate the minimal `Storage` interface from the richer content/object/manifest layers on top.** Kopia's layered architecture lets it swap backends without touching the snapshot logic.

#### Tech stack
- HTTP: backend SDKs (aws-sdk-go, gcs client lib, azblob, ...)
- Crypto: `crypto/aes`, `golang.org/x/crypto/chacha20poly1305`, `crypto/hmac`
- Hashing: `crypto/sha256`, `lukechampine.com/blake3`, Rabin (custom in `repo/splitter/`)
- Compression: `zstd` (`github.com/klauspost/compress`)
- Config: JSON repo manifest + local kopia.config
- Scheduling: cron-like in `snapshot/policy/`

---

## 3. Comparison Tables

### Table A — Authentication approaches

| Product | OAuth flow type | Credential storage | Refresh strategy |
|---|---|---|---|
| **rclone** | Loopback HTTP on 127.0.0.1:53682 for interactive; "manual" copy-paste for headless; service accounts for non-interactive | Single flat file `rclone.conf` (INI), optionally password-encrypted | `golang.org/x/oauth2` auto-refresh wrapping the HTTP client |
| **object_store** | None (no OAuth); IAM/service-account/static creds only | In-memory only; persistence is caller's job | `CredentialProvider` trait with internal `Mutex<(Token, Instant)>` caching |
| **Nextcloud Desktop** | OAuth2 with PKCE, embedded `QWebEngineView` or external browser | OS keychain via `qt5keychain` (Keychain/Credential Manager/Secret Service) | Per-request; on 401 re-run flow and surface tray modal if refresh fails |
| **Syncthing** | N/A — TLS client cert PKI; device ID = SHA-256(cert) | Cert + key files (`cert.pem`, `key.pem`) on disk | N/A — certs don't expire |
| **Cyberduck** | OAuth2 (PKCE where provider supports) via loopback; embedded browser or OS browser | OS keychain (JNA bindings) | Apache HttpClient interceptor auto-refreshes on 401; modal on second 401 |
| **Kopia** | None; static creds / env / cloud SDK default chain | Repository config JSON + cloud SDK's own cred chain | SDK-level (AWS assume-role, GCP token refresh, etc.) |

### Table B — Change detection

| Product | Strategy | Local state schema (gist) | Real-time? | Handles rename? |
|---|---|---|---|---|
| **rclone** | Full recursive list + compare on `(name,size,modtime,hash)` every sync; `bisync` keeps snapshot files | None (stateless) except `bisync` state dir with `.lst` listing snapshots | No (polling or triggered); mount uses `ChangeNotifier` where backend supports | Only with `--track-renames` + hash match |
| **object_store** | Caller's responsibility; primitives: ETag, version, conditional GET/PUT | None | No | No (rename = copy+delete) |
| **Nextcloud Desktop** | WebDAV `PROPFIND` with etag-per-directory; etag propagation prunes walk. Local: DB+FS hybrid driven by file-watcher | SQLite `metadata(path, inode, modtime, size, etag, fileid, content_checksum, remote_perm)` + conflicts, errors, download/uploadinfo | Yes via server push notifications + local fs watchers | **Yes** — via stable `oc-fileid` in journal; issues server-side `MOVE` |
| **Syncthing** | Block-level hashing with weak-hash cache; delta index by `{index_id, max_sequence}` | BoltDB keyed by `folder/device/sequence`; each FileInfo has version vector + block list | Yes — file watcher + periodic scan | Implicit — block dedup makes rename free even without tracking |
| **Cyberduck** | One-shot list compare for sync feature; no persistent state | None (a file browser, not a sync tool) | No | Path-based; some backends expose file_id in `Path.attributes()` |
| **Kopia** | Snapshot-based: walk local tree, hash files, diff block IDs against repo index; `(inode,size,mtime)` cache skips unchanged | Local content cache (LRU) + repo index maps BlockID → pack location | No (scheduled snapshots) | Implicit via CAS — rename is a metadata change only |

### Table C — Backend abstraction

| Product | Interface name | # providers | Add-new-backend complexity |
|---|---|---|---|
| **rclone** | `fs.Fs` (required) + `fs.Features` struct + ~20 optional interfaces (`Copier`, `Mover`, `DirMover`, `ListRer`, `ChangeNotifier`, `Abouter`, `MergeDirser`, `PublicLinker`, ...) | 60+ | Medium: one package under `backend/<name>/`, implement `Fs` + relevant optional interfaces, register in `init()`. Typical backend is 1500-3000 LoC. |
| **object_store** | `ObjectStore` (required, 10 methods) + `ObjectStoreExt` (convenience, blanket-impl) | 5 built-in (AWS, Azure, GCP, HTTP/WebDAV, local) + memory, plus community (HDFS, OpenDAL wrapper) | Low-medium: one file under `src/<name>/`, builder struct, `impl ObjectStore`. Backends are 500-2000 LoC because of shared HTTP/retry/multipart infra in `client::*`. |
| **Nextcloud Desktop** | N/A (single backend); `Vfs` abstract class for VFS plugins | 1 protocol (WebDAV+Nextcloud), 3 VFS modes | N/A |
| **Syncthing** | `lib/fs.Filesystem` for local FS; BEP protocol peers | N/A (all peers are Syncthing) | N/A for backends; new peer implementations must implement BEP |
| **Cyberduck** | `Protocol` + `FeatureFactory.getFeature(Class<T>)` for per-capability interfaces (`Read`, `Write`, `Move`, `Copy`, `Delete`, `Directory`, `Find`, `AttributesFinder`, `Timestamp`, `Touch`, `Upload`, `Download`, `Share`, `Versioning`, `Quota`, `Location`, `AclPermission`, ...) | ~25 protocols + custom profiles | Medium-high: full Maven module, service-loader registration, implement ~15 capability interfaces. Per-backend module is 5-20k LoC. |
| **Kopia** | `repo/blob.Storage` (7 methods) | ~12 native + rclone-wrapper | Very low: one package with `Storage` impl. Typical backend is 400-800 LoC because the interface is so thin. |

---

## 4. Architecture Lessons for Spacedrive

### Patterns Spacedrive should adopt

1. **rclone-style minimal trait + `Features` runtime struct.** Require `list`, `get`, `put`, `delete`, `mkdir`, `rmdir`, `head`. Expose optional capabilities (`copy`, `move`, `rename`, `share_link`, `change_notify`, `server_side_across_backends`) via sub-traits *and* a `Features` bitmask/struct for quick runtime checks. Source: rclone `fs/types.go` + `fs/features.go`.

2. **object_store-style conditional GET/PUT with ETag/version.** First-class support in the core trait for `GetOptions { if_match, if_none_match, range }` and `PutMode { Create, Overwrite, Update(UpdateVersion { e_tag, version }) }`. This gives you free cache coherence, free OCC, and free "don't re-upload if unchanged." Source: `object_store::ObjectStore::get_opts` / `put_opts`.

3. **Cyberduck-style "Account" above "Volume".** The top-level user concept is an account (Google account, Dropbox account, AWS keypair). An account exposes 1..N drives/buckets/shares, and each drive is a volume Spacedrive indexes. One OAuth token unlocks multiple volumes. This matches how users think and avoids making them re-auth per drive.

4. **Nextcloud-style persistent journal with `(local_id, remote_id, etag/version, modtime, size, content_hash)`.** SQLite table per volume. Treat the remote ID as the identity; use path as a secondary index. When any tuple mismatches, emit a `SyncInstruction` (NEW/UPDATE/RENAME/DELETE/CONFLICT). Source: `src/common/syncjournaldb.cpp`.

5. **Provider-native delta APIs where they exist, abstracted as a "change token."** Google Drive `changes.list` returns a `startPageToken`+`newStartPageToken`; OneDrive `/delta` returns a `@odata.deltaLink`; Dropbox `list_folder/continue` returns a `cursor`. Abstract these as `impl ChangeFeed { fn poll(&self, cursor: Option<String>) -> (Vec<Change>, Option<String>) }`. Fall back to list+diff for providers without delta support (S3, generic WebDAV). This is structurally Syncthing's `{index_id, max_sequence}` pattern applied across vendors.

6. **Per-backend pacer with jittered exponential backoff and error-code classification.** Every backend file should define its own `fn should_retry(err) -> RetryDecision { Retry, Fatal, FatalIfOpted(key) }`. Don't try to have a global retry policy. Source: rclone `backend/drive/drive.go shouldRetry`.

7. **rclone-style restricted-character encoding.** Ship a per-backend encoder that maps local-valid-but-remote-invalid characters to Unicode fullwidth equivalents on upload and reverses on download. Otherwise every Windows user who syncs `report:1.doc` to OneDrive breaks. Source: rclone `lib/encoder`.

8. **Syncthing-style weak-hash cache keyed on `(path, mtime, size)`.** Before computing a strong hash, check the cache. Saves 90%+ of hash work on unchanged files. Source: Syncthing `lib/scanner/hashcache.go`.

9. **OS-keychain credential storage.** Never store OAuth refresh tokens in plaintext config. macOS Keychain, Windows Credential Manager, Linux Secret Service via `keyring` crate. Source: Nextcloud Desktop `creds/` + Cyberduck's JNA bindings.

10. **Conflict files with a documented naming convention, not dialog modals.** `foo.txt (conflicted copy from <device> on <date>).txt`. Users learn it; no UI stall. Source: Nextcloud Desktop `ConflictSolver`.

### Patterns Spacedrive should AVOID

1. **Don't build a sync engine inside the backend trait.** Keep the backend trait pure object-storage-like (`list`/`get`/`put`/...), like object_store. The sync/indexing logic belongs one layer up. Mixing causes every new backend to re-implement change detection. Counter-examples: Nextcloud Desktop entangles the sync engine with WebDAV specifics — Spacedrive can't copy that directly.

2. **Don't make server-side-copy a required method.** Only ~60% of cloud backends support it (no Dropbox→Dropbox on the free tier, limited on B2 across regions, nothing cross-provider). Make it an optional capability and fall back to download+upload with a progress callback.

3. **Don't use path as primary identity.** Rename breaks everything if path is your key. Store the backend's opaque file ID (Drive `fileId`, OneDrive `id`, S3 `versionId`-or-`etag`) and treat path as a lookup index.

4. **Don't unify hash algorithms.** Every provider hashes differently (Dropbox custom, OneDrive `quickxor`, B2 SHA-1, S3 MD5-of-parts for multipart). Store the raw hash + the algorithm name, compare only when both sides produce the same algo. rclone's `hash.Set` bitmask is how this is done.

5. **Don't retry everything.** Retrying on 403 `quotaExceeded` burns through the user's quota and eventually gets their app key banned. Classify errors into `Retry | Fatal | UserFixable` and expose the latter two distinctly in the UI. Source: rclone's `stop_on_upload_limit`.

6. **Don't bake OAuth client secrets into the binary in plaintext.** rclone at least obfuscates them (`obscure.MustReveal`); better to require users to bring their own for production use while shipping a low-rate-limit default.

7. **Don't assume atomic rename.** Most object stores don't have it (S3 = copy+delete, Drive = move via parent-change). Your sync logic must handle "file moved, still being uploaded, parent-change commits before upload finishes" windows.

8. **Don't hide Google Docs / shortcuts / Photos.** rclone tried to; now it has 6 flags (`--drive-skip-gdocs`, `--drive-export-formats`, `--drive-skip-shortcuts`, `--drive-skip-dangling-shortcuts`, `--drive-show-all-gdocs`, `--drive-skip-checksum-gphotos`) and a 300-line docs section. Expose these oddities in the UI as first-class.

### Open design decisions this research surfaces

1. **Should Spacedrive have a unified "Account" concept above "Volume"?** Cyberduck says yes, rclone says no (each "remote" is fully standalone). The account model is friendlier but requires token-scope management (what does "this account" mean when a Google account has My Drive + 3 Shared Drives + Shared-With-Me?). **Recommendation: yes, model Account as a first-class domain entity with 1..N Volumes.**

2. **Content-addressable cache between local and cloud?** Kopia shows how powerful CAS is, but it'd require Spacedrive to chunk-and-hash everything uploaded/downloaded. Huge speedup for cross-provider moves and for "this file already exists on another drive" but doubles the engineering scope. **Recommendation: defer to phase 2; keep phase 1 object-layout-preserving (mirror the provider's file tree).**

3. **Delta-feed abstraction vs per-backend change pollers?** A generic `ChangeFeed` trait is elegant but will paper over real differences (Drive page tokens expire, OneDrive delta links can go 410 Gone, Dropbox cursors are durable). **Recommendation: define the trait but allow backends to return richer errors; document the provider quirks in the trait docs.**

4. **VFS / virtual files — first class or phase 2?** Nextcloud's Windows CfAPI + macOS File Provider integration is the gold standard UX but 20k+ LoC of platform-specific glue. rclone has FUSE mount as a simpler cross-platform alternative. **Recommendation: FUSE mount via `rclone mount`-style in phase 1; native placeholder files (CfAPI/File Provider) in phase 2.**

5. **How much of object_store do we reuse vs reimplement?** Pulling in `object_store` gives us S3/Azure/GCS for free (hundreds of thousands of LoC equivalent), but its auth model (IAM/service-account) doesn't fit desktop OAuth. **Recommendation: use `object_store` as the implementation for S3/Azure/GCS, wrap with Spacedrive's OAuth-aware credential provider, and build Google Drive / OneDrive / Dropbox as separate backends since they're fundamentally different APIs (not S3-compatible).**

6. **Single vs per-provider rate limit governors?** rclone's approach (per-backend pacer) means a slow OneDrive can't starve a fast S3. But it complicates global bandwidth caps. **Recommendation: per-backend pacer for request-rate/quota errors, global token bucket for bytes/sec bandwidth cap.**

7. **Where does business-logic extensibility live?** rclone has "virtual backends" (crypt, chunker, compress, combine, union) that wrap other backends. This is a beautiful composition pattern. **Recommendation: adopt this early as a design affordance, even if Spacedrive doesn't ship wrapped backends at launch.**

---

## 5. Proposed next deep-dives

For the Spacedrive implementer, these files are the highest-value reads:

### object_store (Rust) — 40% of the implementer's time
1. [`src/lib.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/lib.rs) — core trait, rationale, upgrade guide. Read the `ObjectStore` trait doc comments in full.
2. [`src/aws/mod.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/aws/mod.rs) — exemplary builder pattern.
3. [`src/aws/credential.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/aws/credential.rs) — chained credential providers.
4. [`src/client/retry.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/client/retry.rs) — backoff + jitter implementation. `RetryConfig` is the pattern.
5. [`src/multipart.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/multipart.rs) + [`src/upload.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/upload.rs) — `MultipartUpload` trait and `WriteMultipart` helper.
6. [`src/path/mod.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/path/mod.rs) — typed path with normalization rules.
7. [`src/parse.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/parse.rs) — URL → store dispatch.
8. [`src/list.rs`](https://github.com/apache/arrow-rs-object-store/blob/main/src/list.rs) — paginated listing abstraction.

### rclone (Go) — 30%
1. [`fs/types.go`](https://github.com/rclone/rclone/blob/master/fs/types.go) — the `Fs`, `Object`, `ObjectInfo`, `Directory`, `DirEntry` hierarchy. Start here.
2. [`fs/features.go`](https://github.com/rclone/rclone/blob/master/fs/features.go) — the `Features` struct (capability flags) + optional-feature interfaces.
3. [`fs/fs.go`](https://github.com/rclone/rclone/blob/master/fs/fs.go) — the error sentinels and `FileExists`, `GetModifyWindow` helpers.
4. [`backend/drive/drive.go`](https://github.com/rclone/rclone/blob/master/backend/drive/drive.go) — a full production backend (~4800 LoC). Specifically study:
   - `init()` registration + `fs.Register`
   - `Options` struct with `config:"..."` tags
   - `NewFs` constructor including team-drive config state machine
   - `shouldRetry` — the canonical per-backend retry classifier
   - `Fs.list` — how they build search queries, handle pagination, fix Google's fast-list bug
   - `Object.Update` — resumable upload with chunks
5. [`lib/pacer/pacer.go`](https://github.com/rclone/rclone/blob/master/lib/pacer/pacer.go) — pacer primitives (token bucket, Google Drive calculator, AWS calculator, etc.).
6. [`lib/oauthutil/oauthutil.go`](https://github.com/rclone/rclone/blob/master/lib/oauthutil/oauthutil.go) — loopback OAuth flow.
7. [`lib/encoder/encoder.go`](https://github.com/rclone/rclone/blob/master/lib/encoder/encoder.go) — restricted-character encoding.
8. [`backend/onedrive/onedrive.go`](https://github.com/rclone/rclone/blob/master/backend/onedrive/onedrive.go) — study the `/delta` change-feed implementation for remote change detection.
9. [`backend/dropbox/dropbox.go`](https://github.com/rclone/rclone/blob/master/backend/dropbox/dropbox.go) — study the `list_folder/continue` cursor pattern.

### Nextcloud Desktop (C++) — 15%
1. `src/libsync/syncengine.h` + `.cpp` — the orchestrator.
2. `src/libsync/discovery.cpp` + `discoveryphase.cpp` — etag-propagation discovery.
3. `src/common/syncjournaldb.cpp` — the SQLite journal schema.
4. `src/libsync/owncloudpropagator.cpp` — the job queue that applies `SyncFileItem` instructions.
5. `src/libsync/propagateupload*.cpp` — chunked upload v2.

### Syncthing (Go) — 10%
1. `lib/protocol/bep_extensions.go` + `lib/protocol/protocol.go` — BEP wire protocol.
2. `lib/scanner/walk.go` — the scanner with weak-hash cache.
3. `lib/db/schema.go` or equivalent — the index DB layout.
4. `lib/versioner/` — version vector logic.
5. [`docs.syncthing.net/specs/bep-v1.html`](https://docs.syncthing.net/specs/bep-v1.html) — the BEP spec.

### Cyberduck (Java) — 3%
1. `core/src/main/java/ch/cyberduck/core/Protocol.java` — the Protocol interface.
2. `core/src/main/java/ch/cyberduck/core/features/` — per-capability feature interfaces.
3. `oauth/src/main/java/ch/cyberduck/core/oauth/OAuth2RequestInterceptor.java` — auto-refresh interceptor pattern.

### Kopia (Go) — 2%
1. `repo/blob/storage.go` — the minimal backend interface.
2. `repo/content/` — CABS layer if you want to explore CAS.
3. Architecture docs: https://kopia.io/docs/advanced/architecture/

---

## 6. Sources

### Primary source code
- rclone: https://github.com/rclone/rclone (MIT, Go)
- object_store: https://github.com/apache/arrow-rs-object-store (Apache-2.0, Rust, v0.13.2)
- Nextcloud Desktop: https://github.com/nextcloud/desktop (GPL-2.0+, C++/Qt)
- Syncthing: https://github.com/syncthing/syncthing (MPL-2.0, Go)
- Cyberduck: https://github.com/iterate-ch/cyberduck (GPL-3.0, Java)
- Kopia: https://github.com/kopia/kopia (Apache-2.0, Go)

### Documentation
- rclone overview (per-backend features matrix): https://rclone.org/overview/
- rclone remote control API: https://rclone.org/rc/
- rclone Google Drive: https://rclone.org/drive/
- object_store docs.rs: https://docs.rs/object_store/latest/object_store/
- Kopia architecture: https://kopia.io/docs/advanced/architecture/
- Syncthing BEP v1: https://docs.syncthing.net/specs/bep-v1.html
- Syncthing device IDs: https://docs.syncthing.net/dev/device-ids.html
- Nextcloud Desktop user manual: https://docs.nextcloud.com/desktop/latest/

### Specific files cited
- rclone core trait: `fs/types.go` https://github.com/rclone/rclone/blob/master/fs/types.go
- rclone Drive backend: `backend/drive/drive.go` https://github.com/rclone/rclone/blob/master/backend/drive/drive.go
- object_store trait: `src/lib.rs` https://github.com/apache/arrow-rs-object-store/blob/main/src/lib.rs
- Nextcloud SyncEngine: `src/libsync/syncengine.h` https://github.com/nextcloud/desktop/blob/master/src/libsync/syncengine.h
- Cyberduck Protocol: `core/src/main/java/ch/cyberduck/core/Protocol.java` https://github.com/iterate-ch/cyberduck/blob/master/core/src/main/java/ch/cyberduck/core/Protocol.java

### Notes on uncertainty
- **Nextcloud Desktop's ARCHITECTURE.md does not exist** (404); the documentation file `src/libsync/libsync.md` is effectively empty. Architectural analysis is derived from reading source file and directory layouts plus header files (`syncengine.h`, public API). Claims about instruction types (`NEW`/`SYNC`/`RENAME` etc.) come from `SyncFileItem::Instruction` enum visible in header + comments; the exact state-machine wiring in the `.cpp` file was not directly read.
- **Syncthing's documentation pages** for `users/scanning.html` and `advanced/folder-hashers.html` return 404 at time of research. Scanner details are inferred from the BEP v1 spec and general knowledge of the project; specific weak-hash implementation details should be verified against `lib/scanner/walk.go`.
- **`arrow-rs/object_store` path returns 404**; the crate was moved to its own repo `apache/arrow-rs-object-store` in 2025. All object_store citations use the new repo URL.
- **Kopia repository server, maintenance, and scheduling** were not examined in depth; lessons drawn are about backend abstraction and CAS layer only.

---

## Final summary

This survey confirms that Spacedrive's cloud-drive problem is well-mapped territory, and the most actionable reference implementations are rclone (for the trait+features pattern and per-backend pacer/retry) and `object_store` (for the idiomatic Rust async trait with conditional-GET/PUT primitives). Nextcloud contributes the persistent-journal and etag-propagation ideas; Syncthing contributes the change-token and weak-hash-cache patterns; Cyberduck contributes the Account-above-Volume UX and per-capability feature interfaces; Kopia contributes the minimal-backend-API + content-addressable-layering insight for phase 2.

Top 5 lessons:

1. **Core trait minimal, capabilities optional.** rclone's `Fs` + `Features` + optional sub-traits is the proven pattern; copy it for Spacedrive's backend trait, using object_store's `ObjectStore` + `ObjectStoreExt` as the Rust shape.
2. **Conditional GET/PUT with ETag/version as first-class trait methods** — not an afterthought. This unlocks caching, OCC, and cheap change detection for free.
3. **Per-backend retry classifier.** Every backend owns its `should_retry(err) -> Retry | Fatal | FatalIfOpted` function. No global retry policy can be correct across Drive+OneDrive+Dropbox+S3.
4. **File identity = backend ID (not path).** Store `(backend_id, file_id, etag/version)` as persistent identity in the journal; path is a secondary index. This makes rename tracking almost free.
5. **"Account" is the right top-level user concept**, exposing 1..N Volumes per account. One OAuth token, multiple drives, matches user mental model (Cyberduck) and amortizes auth UX cost.
