---
id: FILE-003
title: Cloud Volume File Operations — MVP update proposal
for_task: .tasks/core/FILE-003-cloud-volume-file-operations.md
proposed_status: Done
last_updated: 2026-04-18
---

## Proposed acceptance-criteria update

FILE-003's six acceptance criteria against the current MVP branch:

```diff
 ## Acceptance Criteria

-- [ ] User can copy a local file to a cloud volume
-- [ ] User can copy a file from a cloud volume to local storage
-- [ ] User can copy files between two different cloud volumes
-- [ ] Progress is accurately reported for cloud transfers
-- [ ] Transfers can be cancelled mid-operation
-- [ ] Checksum verification works for cloud transfers
+- [x] User can copy a local file to a cloud volume
+- [x] User can copy a file from a cloud volume to local storage
+- [x] User can copy files between two different cloud volumes
+- [x] Progress is accurately reported for cloud transfers
+- [x] Transfers can be cancelled mid-operation
+- [ ] Checksum verification works for cloud transfers (deferred: ETag/content_md5 plumbing, emits warn! today)
```

Five of six are met. The checksum one is deferred with a `TODO(cloud-mvp)` at `core/src/ops/files/copy/strategy.rs` pointing at the pending ETag / `content_md5` work. `verify_checksum: true` currently emits a `warn!` and proceeds on cloud paths — it does not silently claim success.

## Proposed status

`To Do` → `Done` with one unchecked item tracked in the task body. If the team prefers to keep the whole task open until checksum verification lands, the alternative is `In Progress` with five of six checked.

Recommend `Done` because the core coverage (all four shapes across read/write/local↔cloud/cross-backend) is production-ready and the checksum hole is a well-contained follow-up that belongs in its own task.

## Proposed "Next Steps" rewrite

Replace the existing five-item list with:

```markdown
## Next Steps

1. Wire ETag / `content_md5` verification for cloud copies (remove the `warn!` fallback in `CloudCopyStrategy::execute`).
2. Investigate cross-backend server-side copy (S3 bucket A → S3 bucket B via provider-side APIs, falling back to streaming for mismatched providers).
3. Benchmark chunk-size defaults under real cloud round-trip times.
```

## Proposed frontmatter change

- `status: To Do` → `status: Done`
- `last_updated: 2025-10-14` → `last_updated: 2026-04-18`

## Integration test note

The task body mentions `core/tests/test_cloud_file_ops.rs` as the expected integration test. None exists under that name. The effective coverage is:

- `core/src/ops/files/copy/strategy.rs` unit tests against `services::Memory`.
- `core/src/ops/files/copy/routing.rs` routing tests.
- `core/tests/onedrive_end_to_end_test.rs` end-to-end OAuth + add_cloud journey.

If an integration test specifically for cloud file operations is desired, `services::Memory`-based coverage inside `core/tests/` would be straightforward — but it duplicates what the strategy-level unit tests already exercise. Deferring until checksum verification lands is reasonable.
