---
id: CLOUD-003
title: Cloud Storage Provider as a Volume — MVP update proposal
for_task: .tasks/core/CLOUD-003-cloud-volume.md
proposed_status: Done
last_updated: 2026-04-18
---

## Proposed acceptance-criteria update

The current task file has three acceptance criteria. The MVP branch delivers all three.

Diff (intent only — do not edit `.tasks/` directly):

```diff
 ## Acceptance Criteria

 - [x] A user can add an S3 bucket as a new location in their library.
-- [ ] Files can be copied to and from the cloud volume.
+- [x] Files can be copied to and from the cloud volume.
 - [x] The cloud volume can be indexed like any other location.
```

`CloudCopyStrategy` (Set 8a, commit `232e125c4`) implements:

- Same-backend server-side copy with streaming fallback.
- Local → cloud streaming upload.
- Cloud → local streaming download.
- Cross-backend cloud → cloud streaming.

All four shapes are exercised by unit tests in `core/src/ops/files/copy/strategy.rs` (against `services::Memory`) and by the routing tests in `core/src/ops/files/copy/routing.rs`.

## Proposed status transition

`In Progress` → `Done`. CLOUD-003's scope is cloud-as-a-volume; the MVP branch makes OneDrive fully viable, and the S3-family backends were already read/write functional before Set 1. File operations now close the last outstanding acceptance criterion.

## Proposed "Next Steps" rewrite

Replace the existing three-item list with:

```markdown
## Next Steps

1. Roll the OneDrive browser flow pattern out to Google Drive and Dropbox (see `.investigations/cloud-drives/CLOUD-004-oauth-infrastructure-proposal.md` "Next Steps outside CLOUD-004").
2. Close the B2 / Wasabi / DigitalOcean Spaces rehydration catch-all in `VolumeManager::restore_cloud_volumes`.
3. Consume the OneDrive delta stream to skip re-hashing unchanged files during indexing.
```

## Proposed frontmatter change

- `status: In Progress` → `status: Done`
- `last_updated: 2025-10-14` → `last_updated: 2026-04-18`

## Frontend correctness note

The task mentions "Credentials encrypted with XChaCha20-Poly1305 and stored in OS keyring". Actual implementation stores the encrypted blob in the library's SQLite `cloud_credentials` table, not the OS keyring. Recommend correcting in the same edit:

```diff
-    - Credentials encrypted with XChaCha20-Poly1305 and stored in OS keyring
+    - Credentials encrypted with XChaCha20-Poly1305 and stored in the library's SQLite cloud_credentials table
```
