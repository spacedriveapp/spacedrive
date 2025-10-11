<!--CREATED: 2025-08-08-->
# Virtual Sidecar System (VSS)

Status: Draft

## Summary

Virtual Sidecars are derivative artifacts (e.g., thumbnails, OCR text, embeddings, media proxies) that Spacedrive generates and manages without ever mutating the user’s original files. Sidecars are:

- Content-scoped and deduplicated per unique content (content_uuid)
- Stored inside the library’s portable `.sdlibrary` and travel with it
- Generated asynchronously (“compute ahead of time”), and looked up instantly (“query on demand”)
- Designed for cross-device reuse: once generated on any device, they can be reused elsewhere without reprocessing

This document specifies data model, filesystem layout, local presence management, cross-device availability, APIs, and integration points with indexing and jobs.

## Goals

- Zero-copy, original files remain untouched
- Deterministic paths for hot reads (no DB needed for single fetches)
- Fast bulk presence answers via DB (UI grids, batch decisions)
- Content-level deduplication (unique content → shared sidecars)
- Cross-device awareness and transfer using existing pairing/file-sharing
- Continuous consistency between DB index and sidecar folder

## Non-Goals (initial)

- Complex policy engines (we’ll add policies like prefetch later)
- Non-content (entry-level) sidecars beyond metadata manifests (can be added later)

## Data Model

Two tables extend the library database.

### sidecars

One row per content-level sidecar variant.

- id (pk)
- content_uuid (uuid) — FK to `content_identities.uuid`
- kind (text) — e.g., `thumb`, `proxy`, `embeddings`, `ocr`, `transcript`
- variant (text) — e.g., `grid@2x`, `detail@1x`, `1080p`, `all-MiniLM-L6-v2`
- format (text) — e.g., `webp`, `mp4`, `json`
- rel_path (text) — path under `sidecars/` (includes sharding prefixes, e.g., `content/{h0}/{h1}/{content_uuid}/...`)
- size (bigint)
- checksum (text) — optional integrity for the sidecar file
- status (text enum) — `pending | ready | failed`
- source (text) — producing job/agent id or name
- version (int) — sidecar schema/version
- created_at, updated_at (timestamps)

Constraints:

- Unique(content_uuid, kind, variant)

### sidecar_availability

Presence map per device for fast cross-device decisions.

- id (pk)
- content_uuid (uuid)
- kind (text)
- variant (text)
- device_uuid (uuid)
- has (bool)
- size (bigint)
- checksum (text)
- last_seen_at (timestamp)

Constraints:

- Unique(content_uuid, kind, variant, device_uuid)

## Filesystem Layout

Deterministic paths enable zero-DB hot reads.

```
.sdlibrary/
  sidecars/
    content/
      {h0}/{h1}/{content_uuid}/
        thumbs/{variant}.webp
        proxies/{profile}.mp4
        embeddings/{model}.json
        ocr/ocr.json
        transcript/transcript.json
        manifest.json
```

Rules:

- Content-level sidecars only (media derivations attached to unique content)
- Deterministic naming by `{content_uuid}` + `{kind}` + `{variant}`
- A small per-content `manifest.json` may be used for local inspection/debug
- Two-level hex sharding under `content/` to bound directory fanout and keep filesystem operations healthy at scale:
  - `{h0}` and `{h1}` are the first two byte-pairs of the canonical, lowercase hex `content_uuid` with hyphens removed (e.g., `abcd1234-...` → `h0=ab`, `h1=cd`).
  - Shard directories are created lazily; never pre-create the full shard tree.
  - Always use lowercase to avoid case-folding issues on case-insensitive filesystems.
  - Paths remain fully deterministic and require no DB lookup for single-item fetches.

## Local Presence & Consistency (DB FS)

To keep database and sidecar folder consistent:

- Bootstrap scan: On first enable or periodic maintenance, walk the sharded tree under `sidecars/content/`, infer `(content_uuid, kind, variant, format, path)`, compute size (+ optional checksum), and upsert `sidecars` rows with `status=ready`.
- Watcher: Add a library-internal watcher for `sidecars/` to reflect create/rename/delete into `sidecars` in real time. For large batches, the reconcile job (below) covers race conditions.
- Reconcile job: Periodic, compares DB rows to FS state, repairs drift (e.g., recompute checksum, remove stale DB rows, re-run generation if missing), and updates `sidecar_availability` for the local device.

## Intelligence Queueing (Post Content Identification)

Extend the indexing pipeline with an “Intelligence Queueing Phase” after ContentIdentification:

- For newly created or modified content, enqueue sidecar jobs by type/kind (thumbnails, proxies, embeddings, OCR, transcript, validation hash).
- Job contract (idempotent):
  1. Check DB/FS for existing sidecar → if exists and valid, no-op
  2. Otherwise generate → write file deterministically → upsert `sidecars`(ready)
  3. Update `sidecar_availability` for the local device
- This phase runs asynchronously and never blocks indexing completion

## Cross-Device Availability & Sync

We reuse the pairing + file sharing stack to avoid reprocessing on every device.

- Inventory exchange: Paired devices periodically share compact availability digests for a configured set of sidecar kinds/variants (e.g., thumbnails). For large sets, use chunked lists or Bloom filters per variant.
- Availability updates: On receiving digest, upsert `sidecar_availability(has=true)` with `last_seen_at` for those (content_uuid, kind, variant, device_uuid).
- Sync planner: When UI needs a sidecar and local is missing:
  - Query `sidecar_availability` for candidates on paired devices
  - If present on any device, schedule file transfer for the deterministic path
  - Otherwise schedule local generation
- Transfer path: Use existing file-sharing protocol to fetch `sidecars/content/{h0}/{h1}/{content_uuid}/...`, verify checksum, write locally, upsert `sidecars` and `sidecar_availability(local)`

## Retrieval Strategy

- Single-item fetch (hot path):

  - Compute deterministic path → FS check → return path immediately if exists
  - If missing, schedule generation or remote fetch (async) and return pending handle

- Bulk presence (grids/lists):
  - Query: `SELECT content_uuid, variant FROM sidecars WHERE kind=? AND content_uuid IN (...)` → build presence map
  - Optionally overlay `sidecar_availability` for remote candidates

## APIs (Daemon)

- `sidecars.presence(content_uuids: [], kind: string, variants: []):`
  - Returns `{ [content_uuid]: { [variant]: { local: bool, path?: string, devices: uuid[], status } } }`
- `sidecars.path(content_uuid, kind, variant):`
  - Returns local path if exists; otherwise enqueues generation/transfer and returns a pending token
- `sidecars.reconcile():` triggers reconcile job
- `sidecars.inventory.publish(kind, variants)`: push local availability digest
- `sidecars.inventory.apply(digest)`: apply remote availability update

## Integration Points

- Indexer: Intelligence Queueing Phase dispatch (after content identification)
- Jobs: Sidecar generation jobs per kind/variant; idempotent and fast-path aware
- Watchers: FS watcher on `sidecars/` to keep DB in sync
- Sharing: Use current file sharing protocol for cross-device copies
- Library manager: ensure `sidecars/` directory exists upon library creation

## Status & Integrity

- `status`: `pending | ready | failed` for visibility and retries
- `checksum`:
  - Small files: full hash
  - Large files: optional or size+mtime; verify on transfer/periodic
- `last_seen_at`: for availability freshness and eviction decisions

## Performance Considerations

- Deterministic paths avoid DB lookups for single fetches
- Bulk presence queries avoid N×FS stats
- Background generation keeps UI latency low
- Availability digests prevent wasteful remote checks; sidecars are re-used instead of re-generated

## Phased Rollout

1. Local-only: schema, folder layout, bootstrap scan, watcher, presence API, local generation
2. UI integration: grids use presence API; details use hot path
3. Cross-device: availability exchange, sync planner, transfers; reconcile enhancements
4. Policies: prefetch strategies, priority queues, storage limits

## Open Questions

- Which sidecars are mandatory to sync vs on-demand?
- Retention: when/how to evict large sidecars (proxies) under pressure?
- Security: signed availability digests? Access controls for shared sidecars?

## Appendix: Example Paths

- Grid thumbnail (2x): `sidecars/content/{h0}/{h1}/{content_uuid}/thumbs/grid@2x.webp`
- 1080p proxy: `sidecars/content/{h0}/{h1}/{content_uuid}/proxies/1080p.mp4`
- Embeddings (MiniLM): `sidecars/content/{h0}/{h1}/{content_uuid}/embeddings/all-MiniLM-L6-v2.json`
