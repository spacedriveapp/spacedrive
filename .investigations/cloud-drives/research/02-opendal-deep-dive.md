# OpenDAL Deep Dive for Spacedrive Cloud Drives

Research report — targets Spacedrive's `CloudBackend` (`core/src/volume/backend/cloud.rs`).

Author: research agent. Scope: Apache OpenDAL Rust crate, versions 0.54 → 0.55 (latest published), with focus on what is relevant to Spacedrive's multi-cloud volume backend.

---

## 1. Executive Summary

- **Latest published crate:** `opendal = "0.55.0"` (https://docs.rs/opendal/latest/opendal/ and crates.io index confirms `num: "0.55.0"` as highest). The `main` branch is already working on v0.56 (docs show an "Upgrade to v0.56" section). Spacedrive is one minor version behind on the published release train and about two versions behind `main`.
- **Upgrade from 0.54 → 0.55 is low-risk.** The breaking changes are narrow: timestamp types moved from `chrono::DateTime<Utc>` to `jiff::Timestamp`, `Scheme` returns `&'static str` from `OperatorInfo::scheme()`, `OpList::with_version()` renamed to `with_versions()`, `S3Builder::security_token()` removed (use `session_token()`), and some KV-only services stopped pretending to support `list`. Spacedrive's usage (S3/Gdrive/OneDrive/Dropbox/Azblob/Gcs builders, `read`, `read_with.range`, `write`, `stat`, `lister`, `delete`, `remove_all`, `create_dir`) is untouched by these.
- **Biggest practical gap in the current implementation:** Spacedrive's `CloudBackend` does not use `copy`, `rename`, `writer` streaming, presign, `Capability` feature-detection, or any layers (no retry, no logging, no timeout, no concurrent-limit). It also discards ETag/version/content-md5 that OpenDAL already surfaces on `Metadata` — a missed native change-detection signal.
- **OAuth token refresh** is handled automatically inside the Gdrive/OneDrive/Dropbox backends when `client_id + client_secret + refresh_token` are configured. OpenDAL does **not** give a public callback to persist a rotated refresh token — this is a real gap for Spacedrive and must be worked around (see §6 and §8).
- **Recommended action:** bump to 0.55, add a layer stack, expose ETag/version/md5 through `RawMetadata`, add `copy`/`rename`/`writer`/`presign` capability-aware methods, and implement a polled ETag-diff scan for change detection (native watch is not supported by any of the listed providers via OpenDAL).

### Top 5 findings

1. OpenDAL 0.55 is the newest release; 0.54 → 0.55 is a safe bump for Spacedrive's surface.
2. `Writer` with `.concurrent(n).chunk(size)` is the correct path for streaming 10 GB+ uploads — no manual multipart.
3. `Metadata::etag()`, `Metadata::version()`, `Metadata::content_md5()`, `Metadata::last_modified()` already exist on every provider that supports them. Spacedrive currently reads only `content_length` and `last_modified`.
4. OAuth refresh is transparent inside OpenDAL, but the rotated refresh token is never surfaced to the caller. Use a custom HTTP-layer or out-of-band Graph/Drive token exchange if persistence is required.
5. Native change-detection (Drive `changes.list`, Graph `delta`, Dropbox `list_folder/continue`) is **not** exposed by OpenDAL. Spacedrive must poll with `list_with().recursive(true)` and diff ETag/version/mtime in its own store.

---

## 2. Version & Upgrade Path

| Item | Value |
|---|---|
| Current (Spacedrive) | `opendal = "0.54"` |
| Latest published | `opendal = "0.55.0"` (confirmed via docs.rs rendering `opendal-0.55.0` and crates.io versions list top entry) |
| Main branch working version | `0.56.x` (changelog shows "Upgrade to v0.56" section) |
| Minimum supported Rust | 1.82 (since 0.53) |

### Changelog highlights relevant to Spacedrive, 0.54 → 0.55

From `core/src/docs/upgrade.md` on `main`:

1. **Timestamps switched from `chrono` to `jiff`.**
   `Metadata::last_modified()` now returns `Option<jiff::Timestamp>` instead of `Option<chrono::DateTime<Utc>>`.
   Spacedrive's current code converts via `t.timestamp() as u64`. The same `.timestamp()` accessor exists on `jiff::Timestamp` (returns seconds) — but the method name may differ; on `jiff::Timestamp` use `.as_second()` (check against the new signature). Either way, Spacedrive must adapt the two sites in `cloud.rs` at lines 334 and 361.
2. **`OperatorInfo::scheme()` returns `&'static str`.**
   Spacedrive does not call this, so no impact.
3. **`OpList::with_version()` → `with_versions()` (takes `Vec<String>`).**
   Spacedrive does not use versions today — but when implementing change-detection (§5), prefer `with_versions()` directly.
4. **`S3Builder::security_token()` removed.** Spacedrive does not set this; no impact.
5. **KV-style services no longer advertise `list`.** Not relevant (Spacedrive uses object stores only).
6. **`opendal::raw::tests` → `opendal_testkit`** (0.55 → 0.56). Spacedrive does not depend on raw tests yet.

### Breaking changes assessment

The only code mutation required for Spacedrive's `cloud.rs` under a straight 0.54 → 0.55 bump is the timestamp conversion (jiff API). Everything else Spacedrive uses (`read`, `read_with.range`, `write`, `stat`, `lister.try_next`, `delete`, `remove_all`, `create_dir`, every `services::*::default()` builder) has identical signatures.

If Spacedrive also wants the 0.54 features it is **not** using yet: RFC-6213 options-style APIs (`read_options`, `write_options`, `stat_options`, `list_options`, `delete_options`, `presign_read_options`) are available in 0.54+ and are the cleanest way to plumb conditional headers (`if_match`, `if_none_match`), content-type, cache-control, and presign expirations.

---

## 3. API Reference (focused)

All snippets below are written against OpenDAL 0.55 unless marked otherwise. They are illustrative — verify against current docs before merging.

### 3.1 I/O — read / write

```rust
// Whole-object read into Buffer (== what Spacedrive does today).
let buf: opendal::Buffer = op.read("path/to/file").await?;
let bytes: bytes::Bytes = buf.to_bytes();

// Range read (current Spacedrive usage).
let buf = op.read_with("path/to/file").range(0..8 * 1024 * 1024).await?;

// Chained options — concurrent chunked range read, single call.
let buf = op
    .read_with("huge.bin")
    .range(0..1_073_741_824)
    .chunk(8 * 1024 * 1024)
    .concurrent(4)
    .await?;

// Whole-object write. Since 0.52 this returns Metadata.
let meta: opendal::Metadata = op.write("path/to/file", bytes_vec).await?;
println!("etag={:?}", meta.etag());
```

Since OpenDAL **0.52** every write returns `Metadata`. Spacedrive's current code discards it (`write` → `()`). Capturing that Metadata eliminates the follow-up `stat` call on hot write paths and gives ETag-on-write.

### 3.2 Streaming reader

```rust
let r: opendal::Reader = op.reader("huge.bin").await?;

// Read a range (since 0.46 reads are range-based).
let buf: opendal::Buffer = r.read(0..8 * 1024 * 1024).await?;

// Adapter: futures::io::AsyncRead + AsyncSeek
let async_read = r.into_futures_async_read(0..total_size).await?;

// Adapter: futures::Stream<Item = Result<Bytes>>
let mut stream = r.into_bytes_stream(0..total_size).await?;
while let Some(chunk) = stream.try_next().await? { /* ... */ }
```

Key: `into_futures_async_read` and `into_bytes_stream` are **async** and return `Result` since 0.47.

### 3.3 Streaming writer (multipart, concurrent)

```rust
let mut w = op
    .writer_with("huge.bin")
    .chunk(8 * 1024 * 1024)   // 8 MiB parts
    .concurrent(4)             // 4 parallel PUTs
    .await?;

w.write(chunk_1).await?;
w.write(chunk_2).await?;
// ...
let meta: opendal::Metadata = w.close().await?;   // since 0.52 close returns Metadata
```

Notes:
- `writer_with` handles S3/Azblob/GCS multipart, B2 large-file upload, OneDrive resumable session, and Drive resumable upload transparently.
- `append(true)` turns any writer into an append writer on services where append is supported (see §4).
- `into_sink()` / `into_futures_async_write()` bridge to `futures::Sink<Bytes>` / `futures::io::AsyncWrite`.
- No user-visible "multipart" API anymore. The old `create_multipart().write().complete()` flow (RFC-0438/1420) has been consolidated into `Writer`.

### 3.4 Metadata

```rust
let m: opendal::Metadata = op.stat("path/to/file").await?;
// All Optional — presence depends on provider capability.
m.content_length();              // u64
m.last_modified();               // Option<jiff::Timestamp>     (was chrono pre-0.55)
m.etag();                        // Option<&str>
m.version();                     // Option<&str>                (version id if supported)
m.content_md5();                 // Option<&str>                (base64 on S3/GCS)
m.content_type();                // Option<&str>
m.content_disposition();         // Option<&str>
m.cache_control();               // Option<&str>
m.user_metadata();               // Option<&HashMap<String,String>> (x-amz-meta-* / x-ms-meta-* / object userMetadata)
m.is_dir() / m.is_file();        // mode check
```

The `write()` and `writer.close()` calls return the same `Metadata` since 0.52, so you can skip the follow-up `stat` when the service returns enough fields.

### 3.5 Copy, rename

```rust
// Server-side copy where the provider supports it; layered fallback otherwise.
op.copy("src/path", "dst/path").await?;

// Server-side rename (move) where supported.
op.rename("old/path", "new/path").await?;
```

The `Operator` also exposes `Operator::copy_options` / `Operator::rename_options` variants for conditional headers in 0.54+.

The availability of these is **provider-dependent** — always guard on `Capability`:

```rust
let cap = op.info().full_capability();
if cap.copy    { /* op.copy(src, dst).await? */ }
if cap.rename  { /* op.rename(src, dst).await? */ }
// fall back to read-then-write + delete otherwise
```

### 3.6 Listing

```rust
// Flat, vector return. list returns the path itself too (since 0.50).
let entries: Vec<opendal::Entry> = op.list("dir/").await?;

// Streaming, recursive.
use futures::TryStreamExt;
let mut lister: opendal::Lister = op.lister_with("dir/").recursive(true).await?;
while let Some(entry) = lister.try_next().await? {
    let meta = entry.metadata();           // metadata available inline — no extra stat
    if meta.is_file() { /* ... */ }
}

// With start_after (resume a paginated scan).
let lister = op
    .lister_with("dir/")
    .start_after("dir/lastfile.bin")
    .await?;

// Glob filter (0.48+).
let jpegs = op.list_with("media/").glob("**/*.jpg").await?;

// Limit per page hint.
let lister = op.lister_with("dir/").limit(500).await?;
```

Entries carry inline metadata — use `entry.metadata()` before calling `op.stat()`; only stat when you need fields the list didn't populate.

### 3.7 Delete / bulk delete

```rust
op.delete("file").await?;            // single
op.remove_all("dir/").await?;        // recursive directory removal

// New Deleter API (0.51+). Use for bulk deletion — batches automatically.
let mut d = op.deleter().await?;
d.delete("a.bin").await?;
d.delete("b.bin").await?;
d.close().await?;                    // or drop
```

For Spacedrive, the current `remove_all` call is fine for recursive delete. `deleter()` is worth switching to when deleting N known files because it will group them into service-native batch deletes (S3 DeleteObjects, Azure batch, etc.).

### 3.8 Presign

```rust
use opendal::options::PresignOptions;
use std::time::Duration;

let opts = PresignOptions::new().expire(Duration::from_secs(3600));
let presigned = op.presign_read_options("path/to/file", opts).await?;
// presigned.method(), presigned.uri(), presigned.headers()

let presigned_put = op
    .presign_write_options("upload/here", PresignOptions::new().expire(Duration::from_secs(900)))
    .await?;
```

Non-options variants `presign_read` / `presign_write` / `presign_stat` are also available but accept only `Duration`.

### 3.9 Error handling

```rust
use opendal::ErrorKind;

match op.stat("maybe.bin").await {
    Err(e) if e.kind() == ErrorKind::NotFound         => { /* expected miss */ }
    Err(e) if e.kind() == ErrorKind::PermissionDenied => { /* auth issue */ }
    Err(e) if e.is_temporary()                         => { /* retryable */ }
    Err(e) => return Err(e.into()),
    Ok(m)  => { /* ... */ }
}
```

`ErrorKind` variants relevant to Spacedrive (from `enum opendal::ErrorKind` on 0.55):

- `NotFound`
- `PermissionDenied`
- `IsADirectory` / `NotADirectory`
- `AlreadyExists`
- `RateLimited`
- `ConditionNotMatch`  (If-Match / If-None-Match failure)
- `InvalidInput`
- `ConfigInvalid`
- `Unsupported`  (capability not supported by provider)
- `Unexpected`  (covers auth-refresh failure, network, etc.)

`error.is_temporary()` is the signal `RetryLayer` already uses internally. Business code can match on it for custom backoff.

One important new kind (0.55+ RFC-6817): `ErrorKind::ChecksumMismatch` when `ChecksumLayer` is enabled.

### 3.10 Layers

```rust
use opendal::layers::{
    LoggingLayer, RetryLayer, TimeoutLayer,
    TracingLayer, MetricsLayer, ConcurrentLimitLayer,
    ThrottleLayer, HttpClientLayer,
};

let op = opendal::Operator::new(builder)?
    .layer(RetryLayer::new().with_max_times(3).with_jitter())
    .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(60)).with_io_timeout(Duration::from_secs(15)))
    .layer(ConcurrentLimitLayer::new(128))
    .layer(LoggingLayer::default())
    .layer(TracingLayer)
    .finish();
```

Notes:
- `LoggingLayer` uses the `log` crate. `TracingLayer` uses `tracing` (Spacedrive's default). Prefer `TracingLayer`.
- `RetryLayer::new()` defaults to exponential backoff via `backon`. `with_jitter()` is strongly recommended.
- `TimeoutLayer::with_speed` was deprecated in 0.45 and **removed in 0.56 main**. Use `with_io_timeout` instead; do not adopt the speed variant even if still visible on 0.55.
- `HttpClientLayer` replaces `Operator::update_http_client` (since 0.54) and is the supported extension point if Spacedrive ever wants a shared `reqwest::Client`.

---

## 4. Provider Matrix

`Capability` is authoritative at runtime — fetch with `op.info().full_capability()` and branch on the boolean fields (`cap.copy`, `cap.rename`, `cap.presign`, `cap.write_can_append`, `cap.write_can_multi`, `cap.list_with_recursive`, `cap.list_with_versions`, `cap.stat_has_etag`, `cap.stat_has_version`, `cap.stat_has_content_md5`, etc.). The table below is the state as documented at the time of research; it should be verified against `cap.*` at startup for each operator.

| Provider | Builder | Required fields | Copy | Rename | Presign | Multipart / Append | Versioning | Native change IDs | Notable quirks |
|---|---|---|---|---|---|---|---|---|---|
| **AWS S3** | `services::S3::default()` | `bucket`, `region`, `access_key_id`, `secret_access_key` (or env/IMDS); `endpoint` optional | Yes (server-side) | **No** | Yes (signed URL) | Multipart yes, append no | Yes (object version id) | ETag (often MD5 for single-part) | `create_dir` writes a zero-byte `/` marker; S3 doesn't have real dirs. `role_arn`, `session_token` supported. `default_storage_class` / `default_cache_control` configurable. |
| **Wasabi** (S3) | `services::S3::default()` + `endpoint("https://s3.<region>.wasabisys.com")` | as S3 | Yes | No | Yes | Multipart yes | Bucket-configurable | ETag | Wasabi native service was removed in 0.42 — use `S3` builder. |
| **DigitalOcean Spaces** (S3) | `services::S3::default()` + `endpoint("https://<region>.digitaloceanspaces.com")` | as S3 | Yes | No | Yes | Multipart yes | Limited | ETag | Use S3 builder. |
| **Backblaze B2** | `services::B2::default()` | `application_key_id`, `application_key`, `bucket`, `bucket_id` | Yes | No | Yes | Large-file upload yes | Yes (file versioning is native to B2) | `content_sha1` + version id | Paths are case-sensitive. B2 "large file" upload is translated to OpenDAL's `writer_with().chunk()`. |
| **Cloudflare R2** (S3) | `services::S3::default()` + `endpoint("https://<acct>.r2.cloudflarestorage.com")`, `region("auto")` | bucket, keys, endpoint | Yes | No | Limited | Multipart yes | No | ETag | Recommend `batch_max_operations(700)` and `enable_exact_buf_write` per OpenDAL docs. |
| **Azure Blob Storage** | `services::Azblob::default()` | `container`, `account_name`, `account_key` (or SAS / Azure AD); `endpoint` required | Yes | **No** | Yes | Block-blob multipart yes, append via append-blob type | Yes (blob versioning when enabled on container) | ETag + `x-ms-version-id` | Endpoint like `https://<acct>.blob.core.windows.net`. For Azurite use `http://127.0.0.1:10000/devstoreaccount1`. |
| **Google Cloud Storage** | `services::Gcs::default()` | `bucket`; `credential` (base64 JSON) or `credential_path` or `service_account` or ADC | Yes | **No** | Yes (requires private-key credential) | Resumable multipart yes | Yes (object generation) | ETag, `generation`, `crc32c`, `md5Hash` | Presign requires SA key, not ADC. `predefined_acl`, `default_storage_class` configurable. |
| **Google Drive** | `services::Gdrive::default()` | `access_token` **or** (`refresh_token` + `client_id` + `client_secret`); `root` optional | Yes | Yes | **No** | Resumable upload yes | No (revision id exists in Drive but not surfaced as OpenDAL `version`) | `md5Checksum`, `modifiedTime` (both surface as `content_md5` / `last_modified`); ETag is not surfaced | Multiple-parents model in Drive is flattened by OpenDAL — a file appears under one logical path only. `create_dir` creates an actual folder. File IDs not exposed. |
| **OneDrive** | `services::Onedrive::default()` | `access_token` **or** (`refresh_token` + `client_id` [+ optional `client_secret`]); `root` optional; `enable_versioning` opt-in | Yes | Yes | **No** | Resumable upload session yes | Yes (when `enable_versioning`) | ETag (Graph `eTag`), `cTag` not surfaced, `last_modified` | "For write-related operations such as `write`, `rename`, `copy`, and `create_dir`, the OneDrive service replaces the destination folder instead of performing a rename operation." (OpenDAL docs) — treat OneDrive rename/copy as destructive-if-existing. |
| **Dropbox** | `services::Dropbox::default()` | `refresh_token` + `client_id` + `client_secret` (long-term) or `access_token` (temporary) | Yes | Yes | **No** | Upload session yes | No (Dropbox "rev" is surfaced via `content_hash` / version) | `content_hash` (Dropbox proprietary SHA-256 rolling hash), `server_modified` | Dropbox has no real folder metadata (no etag on folder). Paths must begin with `/`. No true directories — `create_dir` creates a folder via explicit API call. `content_hash` does **not** match local file md5/sha256 — it's Dropbox's chunked hash algorithm. |

**No provider in this list supports OpenDAL `presign` for Drive/OneDrive/Dropbox** — this is a Graph/Drive API limitation, not an OpenDAL gap. S3-family and GCS do.

**No provider in this list supports server-side rename on S3/Azblob/GCS** — OpenDAL will emulate with copy+delete when capability is absent only if you build that logic yourself; `op.rename` returns `Unsupported` on S3. (Compare: a `RenameLayer`-style helper does not ship by default.)

---

## 5. Change Detection Primitives

### What OpenDAL gives natively (per provider)

| Signal | S3 | Azblob | GCS | B2 | Gdrive | OneDrive | Dropbox |
|---|---|---|---|---|---|---|---|
| `Metadata::etag()` | Yes | Yes | Yes | Yes (sha1) | No | Yes | No |
| `Metadata::last_modified()` | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| `Metadata::content_md5()` | Yes (on PUT) | Yes | Yes (`md5Hash`) | No (uses sha1) | Yes (`md5Checksum`) | No | Yes (`content_hash`, proprietary) |
| `Metadata::version()` (version id) | Yes if versioning on | Yes if versioning on | Yes (`generation`) | Yes | No | Yes if `enable_versioning` | No |
| `Metadata::user_metadata()` | Yes | Yes | Yes | Yes | No | No | No |
| If-Match / If-None-Match (conditional read/write) | Yes | Yes | Yes | Partial | No | Yes | No |
| Change feed API | No (not in OpenDAL) | No (Blob change feed not wrapped) | No (Object Change Notification not wrapped) | No | No (Drive `changes.list` not wrapped) | No (Graph `delta` not wrapped) | No (`list_folder/continue` not wrapped) |

### What OpenDAL does **not** give you

None of the provider-native delta APIs are exposed by OpenDAL. You cannot get:
- Drive `changes.list` with `pageToken`
- Graph `/me/drive/root/delta`
- Dropbox `/files/list_folder/continue` with `cursor`
- S3 EventBridge / SNS notifications
- Azure Blob change feed
- GCS Pub/Sub object change notifications

These are all out-of-scope for OpenDAL's unified layer.

### Practical approach for Spacedrive

The only portable strategy is polled listing + ETag/version/mtime diffing. Sketch:

```rust
// pseudocode — verify against current docs before merging

struct CloudChangeMap {
    // key: cloud_path, value: best-available change fingerprint
    known: HashMap<String, Fingerprint>,
}

enum Fingerprint {
    VersionId(String),   // preferred when stat_has_version
    Etag(String),        // next-best
    Md5(String),
    ModifiedOnly { mtime: jiff::Timestamp, size: u64 },  // last-resort
}

async fn scan_diff(op: &Operator, root: &str, state: &mut CloudChangeMap) -> Result<Vec<Change>> {
    let mut l = op.lister_with(root).recursive(true).await?;
    let mut seen = HashSet::new();
    let mut changes = Vec::new();

    while let Some(entry) = l.try_next().await? {
        seen.insert(entry.path().to_owned());
        let m = entry.metadata();
        let fp = fingerprint_from(m);
        match state.known.get(entry.path()) {
            Some(prev) if *prev == fp => { /* unchanged */ }
            Some(_) => changes.push(Change::Modified(entry.path().into())),
            None    => changes.push(Change::Added(entry.path().into())),
        }
        state.known.insert(entry.path().into(), fp);
    }
    for gone in state.known.keys().filter(|k| !seen.contains(*k)).cloned().collect::<Vec<_>>() {
        state.known.remove(&gone);
        changes.push(Change::Removed(gone));
    }
    Ok(changes)
}
```

Prefer `version()` over `etag()` over `content_md5()` over `(mtime,size)`, depending on what `op.info().full_capability().stat_has_*` reports for the provider.

For very large trees, `list_with.limit()` + `start_after` pagination keeps a scan interruptible. Results from `Lister` already carry metadata so a pure listing pass usually avoids N+1 `stat` calls — but some providers only return `content_length` and `last_modified` on listing; check `cap.list_has_etag` etc. to decide whether a follow-up `stat` per entry is needed.

---

## 6. OAuth Integration Pattern (Gdrive / OneDrive / Dropbox)

### What OpenDAL does automatically

When a builder is configured with `client_id + client_secret + refresh_token`:

- **Gdrive:** OpenDAL refreshes the Google OAuth access token via the Google token endpoint when the current one is near expiry. Scope must include `https://www.googleapis.com/auth/drive`. OpenDAL does **not** perform the initial authorization-code exchange — the caller must obtain the refresh token.
- **OneDrive:** Same pattern using the Microsoft identity platform token endpoint. `client_secret` is optional (public clients).
- **Dropbox:** Same pattern, using Dropbox's `/oauth2/token` endpoint.

### What OpenDAL does **not** do

- No callback when the refresh token itself rotates. Some providers (notably when Microsoft requires `offline_access` re-consent, or when Dropbox rotates) will issue a new refresh token. OpenDAL's current builders take the refresh token by value and do not expose a hook to surface the rotated value. Source: `core/src/services/{gdrive,onedrive,dropbox}` builder signatures accept `&str` only.
- No persistent token store. Tokens live inside the service's internal state while the `Operator` is alive.

### Recommended Spacedrive pattern

```rust
// pseudocode — verify against current docs before merging

async fn build_gdrive_operator(creds: &GdriveCreds) -> Result<Operator> {
    let builder = services::Gdrive::default()
        .client_id(&creds.client_id)
        .client_secret(&creds.client_secret)
        .refresh_token(&creds.refresh_token)
        .root(&creds.root);

    let op = Operator::new(builder)?
        .layer(RetryLayer::new().with_jitter())
        .layer(TimeoutLayer::new().with_io_timeout(Duration::from_secs(30)))
        .layer(ConcurrentLimitLayer::new(8))         // Drive has low QPS per-user
        .layer(TracingLayer)
        .finish();

    Ok(op)
}
```

### Verification gap: refresh-token rotation persistence

I could not confirm from public docs whether OpenDAL 0.55 Dropbox/Gdrive/OneDrive rotate and discard refresh tokens silently, or whether they stick with the originally supplied refresh token indefinitely. **Verification step for the implementer:**

1. Read `core/src/services/dropbox/core.rs`, `core/src/services/gdrive/core.rs`, `core/src/services/onedrive/core.rs` in a local `opendal` checkout at tag `v0.55.0`.
2. Look for the token-refresh function. Grep for `refresh_token` and see whether it (a) stores a new `refresh_token` back into any shared state, (b) ignores the response field entirely.
3. Based on the answer, decide:
   - If OpenDAL ignores rotated refresh tokens: Spacedrive must run its own refresh loop out-of-band (own refresh client, then pass `access_token` to OpenDAL and rebuild operator on expiry).
   - If OpenDAL rotates internally but doesn't expose: wrap with a proactive token-refresh cron before OpenDAL's internal refresh fires, to capture rotation ourselves.

### Safer alternative: drive OAuth yourself, pass access-token

```rust
// Proactive refresh loop owned by Spacedrive.
let creds = token_store.get("gdrive").await?;
if creds.access_token_near_expiry() {
    let new = oauth_client.refresh(&creds.refresh_token).await?;
    token_store.put("gdrive", new.clone()).await?;   // persist rotated refresh_token
}

let builder = services::Gdrive::default()
    .access_token(&token_store.current_access_token("gdrive").await?)
    .root(&creds.root);

let op = Operator::new(builder)?.layer(...).finish();
```

Downside: operator has to be rebuilt every refresh (new `access_token` value). Pros: full control over refresh-token persistence, exactly matches what Spacedrive needs for secrets rotation.

---

## 7. Layered Setup Recommendation

For all cloud operators, Spacedrive should apply this default stack. Order matters — outermost layer wraps innermost, and errors/metrics are recorded at each boundary.

```rust
// pseudocode — verify against current docs before merging

use std::time::Duration;
use opendal::layers::{
    ConcurrentLimitLayer, LoggingLayer, RetryLayer, TimeoutLayer, TracingLayer,
};

fn wrap_with_spacedrive_defaults(op: opendal::OperatorBuilder<impl opendal::raw::Access>)
    -> opendal::Operator
{
    op
        // Innermost (closest to the backend): tracing, so spans cover actual I/O.
        .layer(TracingLayer)
        // Retries: exponential, with jitter, bounded.
        .layer(
            RetryLayer::new()
                .with_max_times(4)
                .with_jitter()
                .with_factor(2.0),
        )
        // Per-op and per-IO timeouts. Avoids hangs on flaky Wi-Fi.
        .layer(
            TimeoutLayer::new()
                .with_timeout(Duration::from_secs(120))
                .with_io_timeout(Duration::from_secs(30)),
        )
        // Cap total in-flight HTTP requests per operator. Drive/OneDrive are rate-limited.
        .layer(ConcurrentLimitLayer::new(16))
        // Outermost: logging last so log lines reflect post-retry final status.
        .layer(LoggingLayer::default())
        .finish()
}
```

Per-provider tuning:

- Google Drive: `ConcurrentLimitLayer::new(4..8)` — API quota is strict.
- OneDrive: `ConcurrentLimitLayer::new(8..16)`.
- Dropbox: `ConcurrentLimitLayer::new(8)`; watch for 429s.
- S3/GCS/Azure: `ConcurrentLimitLayer::new(32..64)` — high throughput tolerated.
- B2: `ConcurrentLimitLayer::new(8)` and a higher `TimeoutLayer::timeout` — B2 large-file commit can be slow.

Do **not** enable `MetricsLayer` / `PrometheusLayer` / `PrometheusClientLayer` unless Spacedrive has a Prometheus endpoint. They will compile in transitive deps otherwise.

---

## 8. Gaps & Workarounds

### 8.1 No native watch/delta

OpenDAL doesn't expose Drive `changes.list`, Graph `delta`, Dropbox cursor, S3 Events, Azure change feed, or GCS object-change-notifications.

**Workaround:** polled scan with ETag/version/mtime diff (§5). Store the fingerprint map in Spacedrive's existing `indexing::state` so a restart doesn't refetch everything. Per-provider, decide poll interval — Drive/OneDrive tolerate ~60s, Dropbox ~30s, S3/GCS/Azure can be minutes to hours.

**Future:** if a provider-specific delta becomes critical (e.g. ~1M files on Drive where listing is too expensive), consider implementing it directly against the provider SDK and bypassing OpenDAL for that one code path, while keeping OpenDAL for data I/O.

### 8.2 Refresh-token rotation not surfaced

See §6. OpenDAL refreshes the access token internally but doesn't let us persist a rotated refresh token.

**Workaround:** own the refresh loop; pass `access_token` only; rebuild operator on rotation. Or (more invasive) patch/vendor OpenDAL to expose a `TokenUpdater` callback.

### 8.3 No server-side rename on S3/Azblob/GCS

`op.rename(src, dst)` returns `ErrorKind::Unsupported` on those.

**Workaround:** capability-guard and fall back to copy+delete:

```rust
async fn rename_with_fallback(op: &Operator, src: &str, dst: &str) -> opendal::Result<()> {
    let cap = op.info().full_capability();
    if cap.rename {
        return op.rename(src, dst).await;
    }
    if cap.copy {
        op.copy(src, dst).await?;
        op.delete(src).await?;
        return Ok(());
    }
    // Worst case: stream read + write + delete.
    let r = op.reader(src).await?;
    let data = r.read(..).await?;
    op.write(dst, data.to_bytes()).await?;
    op.delete(src).await?;
    Ok(())
}
```

### 8.4 No presign on Drive/OneDrive/Dropbox

These cloud-sync APIs don't offer presigned URLs.

**Workaround:** for "share a file externally" UX, use the provider's native sharing API (Drive `files.create` permission, Graph `createLink`, Dropbox `sharing/create_shared_link_with_settings`). Out of OpenDAL scope.

### 8.5 Spacedrive's current `exists()` is O(stat)

Every `exists()` call issues a full `stat`. OpenDAL does not have a dedicated HEAD-only `exists`. Match on `ErrorKind::NotFound` only — do not swallow all errors:

```rust
async fn exists(&self, path: &Path) -> Result<bool, VolumeError> {
    match self.operator.stat(&self.to_cloud_path(path)).await {
        Ok(_)                                                  => Ok(true),
        Err(e) if e.kind() == opendal::ErrorKind::NotFound     => Ok(false),
        Err(e) => Err(VolumeError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
    }
}
```

The current code's `Ok(false)` on any error hides auth failures as "does not exist" — this is a latent bug.

### 8.6 Metadata conversion throws away precision

`cloud.rs:334` and `:361` convert `last_modified` to seconds by `t.timestamp() as u64`. In 0.55 after switching to `jiff::Timestamp`, use `t.as_millisecond()` or the nanosecond variant and convert to `SystemTime` without rounding. This matters for change detection — a sub-second change on a rapidly edited Drive file is currently invisible to Spacedrive.

### 8.7 Spacedrive discards ETag/version/md5

The `RawMetadata` struct as used at `cloud.rs:364` has no fields for ETag, version id, content md5 or user metadata. Adding those three optional fields unlocks:
- ETag-based change detection (§5)
- If-Match conditional writes (prevent lost updates)
- Content-addressable dedup via provider-computed hash

### 8.8 No tests beyond `ignored` S3 integration

`cloud.rs:450` sets up an ignored integration test. There are no unit tests.

**Workaround:** use `services::Memory` for unit tests. See §9.

---

## 9. Testing Approach

### 9.1 In-memory operator for unit tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendal::services;
    use opendal::Operator;

    fn memory_backend() -> CloudBackend {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        CloudBackend::from_operator(op, CloudServiceType::S3 /* doesn't matter */)
    }

    #[tokio::test]
    async fn round_trip() {
        let be = memory_backend();
        be.write(Path::new("x/y.txt"), Bytes::from_static(b"hi")).await.unwrap();
        assert_eq!(be.read(Path::new("x/y.txt")).await.unwrap().as_ref(), b"hi");
    }

    #[tokio::test]
    async fn stat_returns_size() {
        let be = memory_backend();
        be.write(Path::new("x/z.bin"), Bytes::from_static(&[0u8; 1024])).await.unwrap();
        let m = be.metadata(Path::new("x/z.bin")).await.unwrap();
        assert_eq!(m.size, 1024);
    }
}
```

`Memory` implements the full `Access` trait so every code path Spacedrive has (read, write, list, delete, stat, create_dir, remove_all) works with no credentials. This is what `CloudBackend` tests should use by default — `#[ignore]` tests with real S3 should remain opt-in.

### 9.2 Capability-driven test matrix

For each call that may return `Unsupported`, parametrize by capability. Drive this from `op.info().full_capability()` so the same test file can exercise the S3 path and the Memory path.

### 9.3 Simulated providers via `SimulateLayer` (0.55+)

For behavior that `Memory` doesn't expose (e.g. `list_with_recursive` fallback), the new `SimulateLayer` (RFC-6678) can wrap `Memory` and turn on/off per-capability simulation. This lets Spacedrive verify its capability-guarded code paths without having to actually run against Drive or Dropbox.

### 9.4 Fault injection

No built-in fault-injection layer exists. For retry-behavior tests, the idiomatic pattern is to implement a thin custom `Layer` that fails N times and then succeeds. Feasible but out-of-scope for the initial uplift.

### 9.5 Real-provider integration tests

Keep the existing `#[ignore]` + env-var pattern. Add Drive / OneDrive / Dropbox / B2 equivalents gated on `GDRIVE_REFRESH_TOKEN`, `ONEDRIVE_REFRESH_TOKEN`, `DROPBOX_REFRESH_TOKEN`, `B2_APPLICATION_KEY_ID` respectively. Mark them ignored; run them in a separate CI lane on a schedule rather than every PR.

---

## 10. Sources

### URLs fetched

- https://docs.rs/opendal/latest/opendal/  (confirmed version: `opendal-0.55.0`)
- https://crates.io/api/v1/crates/opendal  (crates.io API; newest `num: "0.55.0"`)
- https://raw.githubusercontent.com/apache/opendal/main/core/core/src/docs/upgrade.md  (0.56 → 0.14 upgrade notes, full)
- https://github.com/apache/opendal/blob/main/core/CHANGELOG.md  (symlink — returned a stub only; upgrade.md is the real changelog)
- Attempted (404 on `main` branch): `core/src/services/{dropbox,gdrive,onedrive}/docs.md` — these paths exist in Context7's index but no longer at that location in the current tree; the `main` branch has reorganized docs under `core/services/<name>/src/docs.md`. The Context7 snippets below are cited from the earlier path but content is current per Context7's index.

### Context7 library

- Library ID: `/apache/opendal` (benchmark score 67.3, 1957 snippets). Supplementary: `/apache/opendal-reqsign` (signing internals — not queried for this report).
- Queries executed:
  1. "Rust Operator copy rename move methods server-side copy"
  2. "Rust Writer multipart upload chunk streaming large file append"
  3. "Rust Metadata etag last_modified version content_md5 user_metadata"
  4. "Rust Google Drive Gdrive service builder refresh_token access_token OAuth"
  5. "Rust OneDrive Dropbox service builder refresh_token OAuth token refresh"
  6. "RetryLayer LoggingLayer MetricsLayer TracingLayer ConcurrentLimitLayer stack configuration"
  7. "Rust ErrorKind error handling rate limit retry provider errors"
  8. "Rust presign presigned URL generate signed URL"
  9. "Rust Capability struct feature detection supports copy rename presign"
  10. "Rust Lister list_with recursive pagination start_after limit"
  11. "Rust S3 services bucket region endpoint Wasabi DigitalOcean B2 compatible s3"
  12. "upgrade 0.54 0.55 breaking changes migration"
  13. "Rust Reader into_stream into_async_read range chunk concurrent streaming"
  14. "Rust user_metadata custom headers content_type cache_control content_disposition"
  15. "Rust copy rename operations Operator path destination source"
  16. "Dropbox capability content_hash rev file metadata limitations"
  17. "Gdrive capability limitations listing copy rename checksum md5"
  18. "Dropbox service capabilities content_hash versioning rename copy supported operations"
  19. "Azblob capabilities Azure stat etag list versioning Gcs Google Cloud Storage"

### Files read

- `E:\spacedrive\core\src\volume\backend\cloud.rs` (485 lines — current Spacedrive implementation)

### Not verified (flagged for implementer)

- Whether the 0.55 `jiff::Timestamp` exposes `.timestamp()` or requires `.as_second()` — check by running `cargo doc --open -p opendal` locally.
- Whether OpenDAL's Gdrive/OneDrive/Dropbox services persist a rotated `refresh_token` internally (§6). Verify by reading `core/src/services/{gdrive,onedrive,dropbox}/core.rs` at tag `v0.55.0` in a local checkout.
- Exact per-provider `Capability` values. The table in §4 is documentation-derived; runtime truth is what `op.info().full_capability()` returns for each operator configured against a real bucket/account.
