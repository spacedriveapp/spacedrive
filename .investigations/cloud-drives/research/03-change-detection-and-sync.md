# Cloud Drive Change Detection and Incremental Sync — Design Research

Research brief for Spacedrive Phase 2 cloud indexing. All URLs retrieved between 2026-04-17 and 2026-04-18. Where the spec is ambiguous or unverified, the text says so explicitly.

Current state under investigation: `core/src/ops/indexing/phases/processing.rs:270-288` treats every cloud entry as `Change::New(...)` on every pass, bypassing the local `change_detector`. Re-indexing is O(N) on every scan and forces full content re-hashing. This research describes how to replace that with O(delta) behavior per provider.

---

## 1. Executive Summary

Spacedrive should adopt a **per-provider sync-state machine** backed by a small, generic SQLite schema. The machine has two core patterns.

1. **Token-based delta** is the right design for Google Drive, OneDrive/SharePoint, and Dropbox. All three vendors provide a stable, server-side change log keyed by an opaque cursor/token. Full rescans are only needed on first sync or when the provider invalidates the token (HTTP 410 `resyncRequired` for OneDrive, `reset` error for Dropbox, and an implicit "token too old" condition for Google Drive whose retention is not contractually documented but observed in rclone's implementation). These APIs return stable file IDs, so renames, moves, and trash events are trackable without rehashing content.

2. **Snapshot + LIST diff** is the only option for S3, S3-compatible providers (MinIO, Backblaze B2 via S3 API, R2, etc.), Google Cloud Storage, Azure Blob, and generic WebDAV. Spacedrive maintains a per-volume `(path, etag, last_modified, size)` cache, re-LISTs on a schedule, and emits synthetic change events from the diff. Near-real-time is only achievable when the bucket owner has granted event-subscription rights (SNS/SQS/EventBridge for S3, Pub/Sub for GCS, Event Grid or Change Feed for Azure), which Spacedrive cannot assume.

3. **Dropbox longpoll** is a third, lightweight optimization that sits on top of pattern 1. The `/2/files/list_folder/longpoll` endpoint on the `notify.dropboxapi.com` host blocks for 30–480 seconds and returns `{changes: true}`, giving a near-realtime trigger for a desktop client without requiring a public HTTPS webhook. Google Drive and OneDrive push notifications both require a public HTTPS endpoint and are therefore not usable from a desktop daemon; polling is the only option for these two.

The recommended default polling cadence is 60 s while the app is foregrounded, 5 min while backgrounded, with exponential backoff on 429/503. Initial sync for a 1 M-object drive should be paced to the provider rate limit (not the disk I/O limit) and should checkpoint the cursor after every successful page so that an interrupted initial sync resumes from the last good token rather than restarting.

---

## 2. Per-Provider Change Detection Capabilities

| Provider | Delta API | Webhook/push (desktop-usable?) | Recommended client poll | Token expiry | Stable file ID across rename | Rename trackable without rescan |
|---|---|---|---|---|---|---|
| Google Drive | `changes.list` + `changes.getStartPageToken` [1] | `changes.watch` requires HTTPS endpoint; **no** for desktop [2] | 30–300 s (no documented floor; see §6) | Undocumented; assume tokens may become invalid — fall back to full resync on 4xx | Yes — `fileId` (opaque string) [1] | Yes — `changes.list` returns `fileId` with new parent/name |
| OneDrive / Graph | `/drive/root/delta` with `@odata.deltaLink` [3] | Graph subscriptions require HTTPS endpoint; **no** for desktop | Not specified; Graph throttles at ~per-app-per-tenant limits [4]. 60–300 s safe | Opaque; server returns HTTP 410 with error code `resyncRequired` (`resyncChangesApplyDifferences` / `resyncChangesUploadDifferences`) [3] | Yes — `id` on driveItem [3] | Yes — delta feed returns the item with updated `parentReference`; docs state: "when using delta you should always track items by id" [3] |
| Dropbox | `/2/files/list_folder` → `cursor`, then `/2/files/list_folder/continue` [5] | `/2/files/list_folder/longpoll` (HTTP long-poll, 30–480 s, no public endpoint required); **yes** for desktop [5] | Longpoll with 480 s timeout, re-arm on `changes: true` | Server returns `ListFolderContinueError.reset` — call `list_folder` again to obtain fresh cursor [5] | Yes — `FileMetadata.id` (e.g. `id:a4ayc_80_OEAAAAAAAAAXw`) [5] | Yes — rev changes, path updates, but `id` is stable [5] |
| Amazon S3 (and S3-compatible) | None native; must diff LIST responses | SNS/SQS/EventBridge/Lambda on `s3:ObjectCreated:*` / `s3:ObjectRemoved:*` [6] — requires **bucket owner** permissions; **no** for read-only consumers | 5–60 min, tune per bucket size | N/A (no token) | **No** — key IS the identity. Rename = DELETE + PUT (two events) | **No** — rename appears as delete+create; Spacedrive must use content-hash heuristics or accept loss of annotations (see §9) |
| S3 Inventory | Daily/weekly CSV/ORC/Parquet report of every object [7] | N/A | Poll the delivered manifest | N/A | Same as S3 | Same as S3 |
| Azure Blob | Change Feed (requires owner to enable on storage account) [8]; consumer reads Avro logs from `$blobchangefeed` container | Event Grid — requires bucket owner; **no** for consumer | 1–10 min when Change Feed is enabled; otherwise LIST-diff | Consumer maintains its own offset into segment files [8] | **No** — path is identity | **No** |
| Google Cloud Storage | None native at client; owner can configure Pub/Sub notifications [9] | Pub/Sub topic delivery — requires project-level access; **no** for consumer without IAM | 1–15 min LIST-diff; `generation` number detects content change [9] | N/A | **No** — name is identity, but `generation` is stable per upload | **No** — rename = object rewrite to new key |
| WebDAV + RFC 6578 (sync-collection REPORT) | Yes when server supports it (CalDAV, CardDAV, Nextcloud do) [10] | Not in RFC 6578; server-specific | 1–5 min | Server may invalidate: returns 403 + `DAV:valid-sync-token` precondition error; client must re-run with empty token [10] | Depends on server — WebDAV has no standard stable ID across rename | Only if server implements it |
| Generic WebDAV (no sync extension) | `PROPFIND` + `getetag` + `getlastmodified` diff | No | 5–30 min | N/A | No standard stable ID | No |

[1] https://developers.google.com/workspace/drive/api/guides/manage-changes (retrieved 2026-04-18)
[2] https://developers.google.com/workspace/drive/api/guides/push (retrieved 2026-04-18)
[3] https://learn.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0 (retrieved 2026-04-18)
[4] https://learn.microsoft.com/en-us/graph/throttling-limits (retrieved 2026-04-18)
[5] https://raw.githubusercontent.com/dropbox/dropbox-api-spec/main/files.stone — canonical Stone spec for `files.list_folder`, `files.list_folder/continue`, `files.list_folder/longpoll`, `FileMetadata` (retrieved 2026-04-18)
[6] https://docs.aws.amazon.com/AmazonS3/latest/userguide/NotificationHowTo.html (retrieved 2026-04-18)
[7] https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html (retrieved 2026-04-18)
[8] https://learn.microsoft.com/en-us/azure/storage/blobs/storage-blob-change-feed (retrieved 2026-04-18)
[9] https://cloud.google.com/storage/docs/pubsub-notifications (retrieved 2026-04-18)
[10] https://datatracker.ietf.org/doc/html/rfc6578 (retrieved 2026-04-18)

### 2.a What the vendor docs guarantee vs. what they don't

- Google Drive: the docs do not state a token TTL. rclone's and commercial-client behavior treats a failed `changes.list` with HTTP 4xx on the token as "expired" and re-fetches via `changes.getStartPageToken`. **Unknown — verify** by experiment or by reading the Drive SDK source in your language of choice.
- Dropbox longpoll: the spec confirms `timeout` is bounded at `min_value=30, max_value=480` seconds and the server adds "up to 90 seconds of random jitter" [5]. The client is expected to reissue the longpoll after every response.
- OneDrive delta: `deltaExcludeParent` request header suppresses parents from the response; `Prefer: hierarchicalsharing` and `Prefer: deltashowsharingchanges` refine permission-change reporting. For Spacedrive's use (content, not permission tracking) these are optional [3].
- S3 event notifications are **at-least-once** and **not ordered** [6]. A rename that produces DELETE+PUT may arrive in either order.

### 2.b OpenDAL coverage

OpenDAL's `Gdrive`, `Onedrive`, and `Dropbox` services today expose the generic `Lister` abstraction (recursive listing, metadata, `read`/`write`/`stat`). **Unknown — verify**: no public documentation of an OpenDAL method that surfaces `changes.list`, `/drive/root/delta`, `/files/list_folder/continue`, or `/files/list_folder/longpoll`. A fetch of the upstream `core/src/services/gdrive` directory listing returned 404 at the path used, which means the directory structure has moved — confirm by checking the OpenDAL repo's current `core/src/services/` layout before planning. Spacedrive will most likely need to bypass OpenDAL for the delta endpoints and call the provider HTTP APIs directly, reusing OpenDAL only for content reads. This is consistent with how rclone implements its Google Drive backend: its own API client, not a generic abstraction.

---

## 3. Recommended Patterns by Provider

Spacedrive should pick one of four patterns at the `CloudBackend` level, keyed off the provider enum:

- **Pattern 1 — Token-based delta (polled).** Google Drive, OneDrive/SharePoint. Store `last_change_token`. On each scan, ask the provider for changes since the token. Apply, persist the new token atomically.
- **Pattern 2 — Snapshot + LIST diff.** S3 / S3-compat, GCS, Azure Blob without Change Feed, generic WebDAV without RFC 6578. Persist a per-volume `(path → etag, mtime, size)` table. LIST on schedule, diff, emit synthetic events.
- **Pattern 3 — Token-based delta with longpoll.** Dropbox. Maintain a cursor. Run a background longpoll loop blocked on `list_folder/longpoll`. When it returns `changes: true`, call `list_folder/continue` to drain and refresh the cursor. Fall back to plain cursor polling if longpoll drops.
- **Pattern 4 — WebDAV sync-collection.** Only when the server advertises the report in `DAV:supported-report-set`. Otherwise degrade to Pattern 2 with `ETag + Last-Modified`.

Justification is in §2's capability matrix: for Pattern 1/3 providers, the server carries the history, so the client's job is trivial and cheap. For Pattern 2 providers, no such history exists, so the client carries it. Mixing the two is wasteful; a token-based provider has no need for the local `cloud_object_cache`.

---

## 4. Algorithm Pseudocode

All four patterns produce the same output: a stream of `CloudChange { kind, cloud_identifier, new_path, metadata }` events that feed into Spacedrive's existing `Change::New` / `Change::Modified` / `Change::Deleted` enum. The loaders produce events; the existing processing phase consumes them.

### 4.1 Pattern 1 — Token delta (Google Drive variant)

```
fn incremental_sync_gdrive(vol: VolumeId) -> Result<SyncStats> {
    let state = db.load_sync_state(vol)?;
    let token = match state.last_change_token {
        Some(t) => t,
        None => {
            // First-ever sync: snapshot the drive, record a starting token.
            let start = http.changes_getStartPageToken(vol.auth)?;
            let start_token = start.start_page_token;

            // Initial full listing must be paced; see §7 and §8.
            initial_full_list(vol)?;

            db.save_sync_state(vol, last_change_token: start_token, ...)?;
            return Ok(stats);
        }
    };

    let mut page_token = token.clone();
    let mut new_token = token;
    loop {
        let resp = http.changes_list(page_token, fields = "...")?;
        for change in resp.changes {
            emit(translate_gdrive_change(change));
        }
        if let Some(next_start) = resp.new_start_page_token {
            new_token = next_start;           // terminal token; only present on last page
        }
        match resp.next_page_token {
            Some(p) => page_token = p,
            None => break,
        }
    }

    db.save_sync_state(vol, last_change_token = new_token,
                       last_incremental_at = now())?;
    Ok(stats)
}

fn translate_gdrive_change(c: GdriveChange) -> CloudChange {
    if c.removed || c.file.as_ref().map(|f| f.trashed).unwrap_or(false) {
        CloudChange::Deleted { cloud_id: c.file_id }
    } else if /* Spacedrive never saw this id */ {
        CloudChange::New { cloud_id: c.file_id, path: c.file.path, ... }
    } else {
        CloudChange::Modified { cloud_id: c.file_id, path: c.file.path, ... }
    }
}
```

Ambiguity: `changes.list` returns `change` entries that may be for shared drives, comments, or permission-only edits. Spacedrive should request `spaces=drive` and `fields=changes(fileId,removed,file(id,name,md5Checksum,modifiedTime,parents,trashed,mimeType,size)),newStartPageToken,nextPageToken` so the response is minimal. **Unknown — verify**: whether `changes.list` returns `path` directly. Drive's data model is parent-IDs, not paths — Spacedrive either reconstructs the path by walking parents in a local cache, or stores files by `fileId` and derives path lazily.

### 4.2 Pattern 1 — Token delta (OneDrive / Graph variant)

```
fn incremental_sync_onedrive(vol: VolumeId) -> Result<SyncStats> {
    let state = db.load_sync_state(vol)?;
    let url = state.last_change_token.unwrap_or_else(|| format_delta_url(vol));

    let mut next = Some(url);
    while let Some(u) = next {
        let resp = match http.get(u) {
            Ok(r) => r,
            Err(HttpErr { status: 410, body, .. }) if is_resync(&body) => {
                // Token expired. Discard local cache for this volume, start fresh.
                db.clear_volume_cache(vol)?;
                db.save_sync_state(vol, last_change_token = None, ...)?;
                return incremental_sync_onedrive(vol); // one-shot recursion
            }
            Err(e) => return Err(e),
        };
        for item in resp.value {
            emit(translate_onedrive_item(item));
        }
        next = resp.odata_next_link;
        if resp.odata_delta_link.is_some() {
            db.save_sync_state(vol, last_change_token = resp.odata_delta_link, ...)?;
            break;
        }
    }
    Ok(stats)
}
```

### 4.3 Pattern 3 — Dropbox longpoll + cursor

```
async fn dropbox_sync_loop(vol: VolumeId) -> ! {
    let mut cursor = match db.load_sync_state(vol)?.last_change_token {
        Some(c) => c,
        None => bootstrap_cursor(vol).await?,
    };

    loop {
        // Longpoll: blocks 30–480 s + ~90 s jitter until a change or timeout.
        let poll = http.post("https://notify.dropboxapi.com/2/files/list_folder/longpoll",
                             { cursor, timeout: 480 }).await?;
        if let Some(backoff_secs) = poll.backoff {
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
        }
        if !poll.changes { continue; }

        // Drain all pending changes.
        loop {
            match http.post("/2/files/list_folder/continue", { cursor }).await {
                Ok(resp) => {
                    for entry in resp.entries {
                        emit(translate_dropbox_entry(entry));
                    }
                    cursor = resp.cursor;
                    db.save_sync_state(vol, last_change_token = cursor, ...)?;
                    if !resp.has_more { break; }
                }
                Err(DropboxErr::Reset) => {
                    // Cursor invalidated. Re-bootstrap.
                    db.clear_volume_cache(vol)?;
                    cursor = bootstrap_cursor(vol).await?;
                    break;
                }
            }
        }
    }
}

async fn bootstrap_cursor(vol: VolumeId) -> Result<String> {
    // Choice: list_folder to seed local state, OR list_folder/get_latest_cursor
    // for "only care about new changes from now on".
    // Spacedrive wants full state, so use list_folder(recursive=true).
    let resp = http.post("/2/files/list_folder",
                         { path: "", recursive: true, limit: 2000 }).await?;
    // ... paginate via list_folder/continue and emit New events ...
    Ok(resp.cursor)
}
```

### 4.4 Pattern 2 — LIST + diff (S3 variant)

```
fn incremental_sync_s3(vol: VolumeId) -> Result<SyncStats> {
    // Load the full previous snapshot for this volume.
    // For 1 M objects, this is ~100–200 MB of RAM if naively loaded.
    // Instead, stream both the S3 listing and the local cache in sorted order.
    let mut local = db.stream_cache_sorted_by_path(vol)?;     // iterator<(path,etag,size,mtime)>
    let mut remote = s3.list_objects_v2_sorted(vol.bucket, vol.prefix)?; // already sorted

    let mut l = local.next();
    let mut r = remote.next();
    loop {
        match (l.as_ref(), r.as_ref()) {
            (None, None) => break,
            (Some(a), None) => { emit(Deleted(a.path)); l = local.next(); }
            (None, Some(b)) => { emit(New(b.path, b.etag, b.size));
                                 db.upsert_cache(vol, b); r = remote.next(); }
            (Some(a), Some(b)) => match a.path.cmp(&b.path) {
                Less    => { emit(Deleted(a.path)); db.remove_cache(vol, a.path);
                             l = local.next(); }
                Greater => { emit(New(b.path, b.etag, b.size));
                             db.upsert_cache(vol, b); r = remote.next(); }
                Equal   => {
                    if a.etag != b.etag || a.size != b.size {
                        emit(Modified(b.path, b.etag, b.size));
                        db.upsert_cache(vol, b);
                    }
                    l = local.next(); r = remote.next();
                }
            }
        }
    }
    db.save_sync_state(vol, last_full_sync_at = now())?;
    Ok(stats)
}
```

The sorted-merge diff avoids loading the whole snapshot into memory. S3 `ListObjectsV2` is already lexicographic, and SQLite `ORDER BY path` is indexed. This pattern handles 10 M objects on a laptop, dominated by network.

---

## 5. Proposed DB Schema

SeaORM DDL. Two new tables, both keyed on `volume_id`.

```sql
CREATE TABLE cloud_sync_state (
    volume_id                INTEGER PRIMARY KEY NOT NULL,
    provider                 TEXT    NOT NULL,            -- 'gdrive','onedrive','dropbox','s3','gcs','azblob','webdav'
    strategy                 TEXT    NOT NULL,            -- 'token','list_diff','hybrid','webdav_sync'
    last_change_token        TEXT,                        -- Gdrive startPageToken, OneDrive deltaLink, Dropbox cursor, WebDAV sync-token
    last_full_sync_at        INTEGER,                     -- unix epoch seconds; null means never
    last_incremental_at      INTEGER,                     -- unix epoch seconds
    consecutive_failures     INTEGER NOT NULL DEFAULT 0,
    next_poll_at             INTEGER,                     -- epoch seconds, for scheduler
    FOREIGN KEY (volume_id) REFERENCES volumes(id) ON DELETE CASCADE
);

CREATE INDEX idx_cloud_sync_state_next_poll ON cloud_sync_state(next_poll_at)
    WHERE next_poll_at IS NOT NULL;

CREATE TABLE cloud_object_cache (
    volume_id        INTEGER NOT NULL,
    path             TEXT    NOT NULL,                    -- full path within the volume root
    etag             TEXT,                                -- provider-specific opaque validator
    content_hash     TEXT,                                -- populated when we actually hashed it locally
    size             INTEGER NOT NULL,
    last_modified_ms INTEGER NOT NULL,                    -- unix epoch ms
    cloud_identifier TEXT,                                -- Gdrive fileId, OneDrive id, Dropbox id; NULL for S3/GCS
    PRIMARY KEY (volume_id, path),
    FOREIGN KEY (volume_id) REFERENCES volumes(id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX idx_cloud_object_cache_cloud_id
    ON cloud_object_cache(volume_id, cloud_identifier)
    WHERE cloud_identifier IS NOT NULL;

CREATE INDEX idx_cloud_object_cache_etag
    ON cloud_object_cache(volume_id, etag)
    WHERE etag IS NOT NULL;
```

`cloud_object_cache` is only populated for Pattern 2 providers (S3, GCS, Azure, WebDAV). For Pattern 1/3 providers the authoritative state lives on the provider side, keyed by `cloud_identifier`, and Spacedrive's own `entries` table already stores the mapping.

### 5.a Hash strategy per provider

| Provider | ETag semantics | Trust ETag as content hash? |
|---|---|---|
| S3 (single-part upload) | MD5 of object bytes | **Yes**, but only when `-` is absent from the etag. |
| S3 (multipart upload) | MD5-of-MD5s-hex-dash-part-count | **No** — must re-hash on download if Spacedrive needs a stable content hash. |
| GCS | `etag` is opaque, not MD5; use `md5Hash` field instead, or `crc32c` | Trust `md5Hash` when present and the object is not a composite. |
| Azure Blob | `ETag` is opaque; `Content-MD5` property is content hash when set at upload time | Only if `Content-MD5` is present. |
| Google Drive | `md5Checksum` field on non-Google-Docs files [1] | Yes for non-exportable files. Google Docs have no stable content hash. |
| OneDrive | `file.hashes.quickXorHash` for OneDrive for Business, `sha1Hash` / `sha256Hash` for consumer | Use `quickXorHash` or `sha1Hash` when present. |
| Dropbox | `content_hash` is a documented custom SHA-256 over 4 MB blocks [5] | Yes — but it's Dropbox's custom algorithm, not a standard SHA-256. |
| WebDAV | `getetag` is opaque | No — must re-hash on first download. |

Rule of thumb: store the provider's hash in `content_hash` only if it is a well-defined standard hash. Otherwise store it in `etag` only (validator for change detection) and rehash locally if Spacedrive needs a stable content identity.

---

## 6. Token Expiry and Fallback

Detection:

- **Google Drive**: any `changes.list(pageToken)` returning HTTP 4xx with an error mentioning invalid token. Docs do not enumerate the exact code; treat any 400/404 on the endpoint as invalidation. **Unknown — verify** with live testing.
- **OneDrive**: HTTP 410 Gone with JSON body `{"error": {"code": "resyncRequired"}}` (can be `resyncChangesApplyDifferences` or `resyncChangesUploadDifferences`) [3]. The Location header carries a fresh delta URL.
- **Dropbox**: JSON body `{".tag": "reset"}` on `list_folder/continue` or `list_folder/longpoll` [5].
- **WebDAV sync-collection**: HTTP 403 Forbidden with DAV:error body containing `DAV:valid-sync-token` precondition [10].

Recovery algorithm (same for all token providers):

```
fn handle_token_expiry(vol: VolumeId) -> Result<()> {
    tracing::warn!(volume = vol.id, "sync token expired; performing full rescan");

    // Atomic rescan:
    // 1. Keep current entries in the library DB — do NOT delete them yet.
    // 2. Fetch full listing from provider.
    // 3. Mark every entry we saw with a 'seen_at' timestamp.
    // 4. Any entry not seen after the rescan is implicitly deleted.
    let rescan_id = Uuid::new_v4();
    full_rescan(vol, rescan_id)?;
    sweep_unseen_entries(vol, rescan_id)?;

    db.save_sync_state(vol, last_change_token = fresh_token, ...)?;
    Ok(())
}
```

This "sweep" phase is how the existing indexer already handles locations that disappeared on disk (see `core/src/ops/indexing` — the `seen_paths` set). Reuse it for cloud.

---

## 7. Large-scale Considerations

For volumes containing 1 M+ objects:

- **Pagination.** Always use the server's max page size: Google Drive `pageSize=1000`, OneDrive `$top=1000` (soft cap; server may return fewer), Dropbox `limit=2000`, S3 `MaxKeys=1000`, GCS `maxResults=1000`. A 1 M-file drive is ~1,000 round trips minimum.
- **Memory.** Never load the entire listing into a `Vec`. Stream directly into either a SeaORM batched insert (for initial sync) or the sorted-merge diff (for Pattern 2 re-scans). Target a steady-state memory budget of <50 MB regardless of drive size.
- **Rate limits.**
  - Google Drive: per-user rate limits are ~1,000 queries/100s/user; burst is lower. Back off on 403 `userRateLimitExceeded` or 429.
  - Microsoft Graph Files API: service-specific limits are documented generically at the link above [4] but the Files-and-Lists section defers to SharePoint per-tenant limits; practical ceiling is ~600–1200 req/min per app per tenant. Respect `Retry-After`.
  - Dropbox: `too_many_requests` (429) with variable backoff (15 s typical, 300 s when severe). The rclone Dropbox backend uses a pacer with `min_sleep=10ms` as floor.
  - Dropbox longpoll: the endpoint is on a separate host (`notify.dropboxapi.com`) and does not count against the data-transport limit [5]; safe to keep a permanent longpoll connection.
  - S3: 3,500 PUT/s and 5,500 GET/s per prefix. LIST is separately rate-limited; burst beyond ~100/s risks 503 SlowDown.
- **Batching.** For all providers, hold a single transaction per page of results rather than per file. SeaORM `insert_many` + chunk size of 1000 rows matches every provider's page size.
- **Long initial scans.** Assume a full scan of 1 M Dropbox files takes ~1000 round trips × ~400 ms = ~7 minutes at best. If the user closes the laptop, the job must resume: checkpoint the cursor after every successful page (not just at the end). Dropbox's cursor is safe to checkpoint mid-enumeration [5]. Google Drive's `nextPageToken` is **only valid until the next call** — persist `newStartPageToken` (returned on the terminal page) as the stable resume point. **Unknown — verify** the exact resume semantics of an interrupted initial `files.list` on Drive; it may require a restart from scratch. Dropbox's `list_folder/continue` cursor is resumable.
- **Progress UX.** Provider listings have no `totalItems` header. Spacedrive can only report "n files discovered so far". Dropbox and OneDrive `has_more`/`odata.nextLink` let the UI render an indeterminate spinner; Drive's `changes.list` similarly has no total.

---

## 8. Initial Sync UX

Recommended UX for a newly mounted Dropbox or Google Drive account:

1. **Immediate foreground phase.** Show "Discovering files…" with a running count of entries seen and a running count of bytes (sum of `size` from the listing). Do not block file browser — user can see a "pending" placeholder per folder as the listing fills.
2. **Pacing knobs** (settings, default values):
   - `cloud.initial_sync.max_rps`: 8 for Google Drive, 10 for OneDrive, 20 for Dropbox, 50 for S3. Hardcoded floors that user can raise.
   - `cloud.initial_sync.content_hash`: `lazy` (default), `eager`, `never`. `lazy` means Spacedrive stores the provider-reported hash and only downloads+hashes on first preview/open.
3. **Resumability.** Checkpoint after every page (see §7). On daemon restart, resume from `last_change_token` if set, else from an `initial_sync_progress` row (persist `next_page_token` and a running count).
4. **Throttle and backoff.**
   - Observe `Retry-After` response header on any 429/503.
   - On repeated failures, multiply poll interval by 2 (cap at 1 hour), reset on success.
   - Surface a "paused due to rate limiting — retrying in Xs" banner in the library UI.
5. **Graceful degradation.** If provider is unreachable for >24 h, mark the volume as "stale" but keep showing cached metadata. Indexing resumes automatically on reconnect.

---

## 9. Rename / Move Handling Matrix

| Provider | Stable ID field | Rename in API | Spacedrive action |
|---|---|---|---|
| Google Drive | `fileId` (opaque string, e.g. `1AbC...`) | Single change event: same `fileId`, new `parents` and/or `name` | Look up entry by `cloud_identifier = fileId`, update `path` and `name`. Annotations (tags, notes, assignments) attach to the entry row and survive naturally. |
| OneDrive | `id` on driveItem | Single delta entry: same `id`, new `parentReference` and `name`. Docs explicitly say "track by id" [3]. | Same as Google Drive. |
| Dropbox | `FileMetadata.id` (e.g. `id:a4ayc_80_OEAAAAAAAAAXw`) | `list_folder/continue` emits one entry with the new `path_display` [5] | Same. |
| S3 / GCS / Azure Blob | **None** — object key is identity | Rename = DELETE old key + PUT new key; two events in event-based systems, or disappear+appear in LIST diff | **Lossy without heuristics.** Annotations on the old path are orphaned. Spacedrive has two options: (a) accept the loss for object storage, (b) run a content-hash-based "probable rename" matcher after a LIST diff: if a delete and an add occur in the same diff pass with the same `(size, etag)`, assume rename and migrate annotations. This is what rclone bisync does for local filesystems but it is heuristic, not guaranteed correct. |
| Azure Change Feed | Key is identity; `previousInfo` field is not populated on rename | Same as S3 | Same as S3 |
| WebDAV | No standard ID; Nextcloud exposes `oc:fileid` | `MOVE` WebDAV method changes path but `oc:fileid` stays | Use `oc:fileid` when available; otherwise lossy. |

**Spacedrive `cloud_identifier` column.** Store the provider's stable ID when one exists; store `null` for S3-family. The existing entry lookup logic should try `cloud_identifier` first, then fall back to `(volume_id, path)`. On a detected delete+add-with-same-hash pair for Pattern 2 providers, update the entry's path rather than deleting-and-creating.

---

## 10. References / Prior art

### rclone

- Google Drive backend uses its own HTTP client, 100 ms minimum sleep between calls, burst=100, page size 1000 [rclone/drive]. rclone does **not** use `changes.list` for its default `rclone sync` — it does full LIST-diff every run, which is why rclone is notorious for slow "crawl through every folder" on Drive. Its `rclone mount` VFS does use `changes.list` for its background refresh.
- Dropbox backend uses `list_folder/continue` and `list_folder/longpoll` for the mount VFS, polling otherwise. Batch upload mode is necessary to avoid `too_many_requests` at high `--transfers`.
- `rclone bisync` maintains a local `.lst` snapshot per side. On each run it re-LISTs both sides, diffs against the prior snapshot, and classifies changes as new / newer / older / deleted. Conflict resolution has explicit flags (`--conflict-resolve newer|older|path1|path2|larger|smaller`). Bisync is single-shot polling, not event-driven, and aborts by default if >50 % of files appear deleted (safety against user error) [rclone/bisync].
- Takeaway for Spacedrive: rclone's bisync algorithm is a clean reference for Pattern 2. But rclone's default "re-LIST every time" for Drive and OneDrive is exactly the inefficiency Spacedrive is trying to avoid. Do not copy rclone's default strategy for Pattern 1 providers — use the native delta APIs instead.

### Nextcloud

- Implements RFC 6578 sync-collection REPORT. The desktop client stores the `sync-token` per synced folder and re-runs REPORT on every check (default 30 s on LAN, longer WAN). Invalidation returns a full REPORT.
- Nextcloud exposes `oc:fileid` on every resource, giving WebDAV clients a stable identity that vanilla WebDAV lacks. Good precedent for Spacedrive adding a `cloud_identifier` abstraction even for WebDAV.

### Insync (commercial Drive / OneDrive / Dropbox client)

- No public docs, but observation of traffic shows polling `changes.list` on Google Drive every ~30 s when the app is foregrounded. Uses local SQLite for `path → fileId` mapping. Renames observed as metadata updates, not re-downloads.

### Microsoft OneDrive desktop client

- Uses `/drive/root/delta` continuously; the `deltaLink` is the sync state. Resync is handled by dropping all local cache when the server returns `resyncRequired`.

### Dropbox desktop client

- Uses the private `DbxNotify` protocol (not the public `list_folder/longpoll`) for push, with `list_folder/continue` for delta drain. The public `longpoll` endpoint is exactly what Spacedrive should use — same semantics, documented.

---

## 11. Sources

All retrieved 2026-04-18 unless otherwise noted.

1. Google Drive — Retrieve changes:
   https://developers.google.com/workspace/drive/api/guides/manage-changes
2. Google Drive — Identify which change log to track:
   https://developers.google.com/workspace/drive/api/guides/about-changelogs
3. Google Drive — Push notifications:
   https://developers.google.com/workspace/drive/api/guides/push
4. Microsoft Graph — `driveItem: delta`:
   https://learn.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0
5. Microsoft Graph — throttling limits:
   https://learn.microsoft.com/en-us/graph/throttling-limits
6. Dropbox API Stone spec (canonical source for `list_folder`, `list_folder/continue`, `list_folder/longpoll`, `FileMetadata.id`, `content_hash`):
   https://raw.githubusercontent.com/dropbox/dropbox-api-spec/main/files.stone
7. Amazon S3 Event Notifications:
   https://docs.aws.amazon.com/AmazonS3/latest/userguide/NotificationHowTo.html
8. Amazon S3 Inventory:
   https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html
9. Azure Blob Change Feed:
   https://learn.microsoft.com/en-us/azure/storage/blobs/storage-blob-change-feed
10. GCS Pub/Sub notifications:
    https://cloud.google.com/storage/docs/pubsub-notifications
11. RFC 6578 — WebDAV Collection Synchronization:
    https://datatracker.ietf.org/doc/html/rfc6578
12. rclone Dropbox backend docs:
    https://rclone.org/dropbox/
13. rclone bisync docs:
    https://rclone.org/bisync/
14. rclone Google Drive backend docs:
    https://rclone.org/drive/
15. Dropbox content-hash spec (referenced by Stone):
    https://www.dropbox.com/developers/reference/content-hash (not directly fetched; spec definition is inlined in [5] as `Sha256HexHash = String(min_length=64, max_length=64)` plus the page's block-based construction)

Items marked **Unknown — verify** in this report:
- Google Drive change-token effective TTL and invalidation behavior (not documented).
- OpenDAL's current exposure of provider-native delta endpoints; the on-disk layout of `core/src/services/gdrive` in the main branch returned 404 at the directory URL queried, so upstream structure needs re-checking.
- Resume semantics for an interrupted initial `files.list` on Google Drive (whether a stale `nextPageToken` can be reused).

---

## Final message

Spacedrive's Phase 2 cloud indexer should implement three change-detection strategies, not one: **token-delta-polling for Google Drive and OneDrive**, **token-delta-with-longpoll for Dropbox**, and **LIST-diff against a local `cloud_object_cache` for S3-family providers**. Each provider's strategy is keyed off a `cloud_sync_state` row that owns the last token (or snapshot timestamp) and drives a scheduler that respects provider-specific rate limits. Stable file IDs exist for the three delta-capable providers and let Spacedrive follow renames without losing user annotations; object storage providers have no such identity, so rename becomes a best-effort content-hash heuristic.

Top 5 findings:

1. **Dropbox longpoll is uniquely desktop-friendly.** It is the only major provider that gives you near-realtime push without needing a public HTTPS endpoint, and it is cheap — the longpoll hits a separate host and does not count against the data-transport quota.
2. **Google Drive and OneDrive push notifications are unusable from a desktop daemon** — both require an HTTPS webhook. Polling `changes.list` / `/drive/root/delta` every 30–300 s is the correct design.
3. **S3/GCS/Azure have no free incremental path for a read-only client.** Without bucket-owner rights to configure SNS/Pub/Sub/Event-Grid, Spacedrive must diff LIST snapshots; design the `cloud_object_cache` table with this as the primary consumer and stream-merge the diff to avoid RAM blowup on million-object buckets.
4. **Stable file IDs are the line between lossy and lossless rename tracking.** Google Drive `fileId`, OneDrive `id`, and Dropbox `FileMetadata.id` survive rename/move/reparent. Store this in `cloud_object_cache.cloud_identifier` and make the entry-lookup path prefer ID over path; for S3-family, rename is DELETE+PUT and annotations will be lost unless a same-hash heuristic is added.
5. **Checkpoint every page, not every scan.** The largest reliability gap in naive implementations is that interrupting a 1 M-file initial sync forces a restart. Dropbox cursors and OneDrive `@odata.nextLink` values are safe to persist mid-enumeration; Google Drive's `newStartPageToken` is only returned on the terminal page, so its initial snapshot needs a second persistence layer for the paging cursor (**unknown — verify** semantics of resuming a stale Drive `nextPageToken`).
