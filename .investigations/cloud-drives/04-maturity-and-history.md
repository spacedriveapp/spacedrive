# Cloud Drives: Maturity, Tests, and Git History

Investigation date: 2026-04-18
Branch: `feature/cloud-drives-investigation` (0 commits ahead of `upstream/main`)
Scope: Read-only audit, no code modifications.

---

## 1. Executive Summary

Cloud drives in Spacedrive sit at roughly **55% of a functional MVP**. The plumbing is real and largely committed to main: an OpenDAL-backed `CloudBackend` (`core/src/volume/backend/cloud.rs:24`), registered library actions `volumes.add_cloud` / `volumes.remove_cloud` (`core/src/ops/volumes/add_cloud/action.rs:505`, `core/src/ops/volumes/remove_cloud/action.rs:75`), encrypted credential storage using XChaCha20-Poly1305 (`core/src/crypto/cloud_credentials.rs:194`), a dedicated SeaORM entity + migration (`core/src/infra/db/entities/cloud_credential.rs`, `core/src/infra/db/migration/m20251204_000001_create_cloud_credentials_table.rs`), an interactive CLI flow (`apps/cli/src/domains/cloud/setup.rs:16`), and a full React modal that can call the action (`packages/interface/src/routes/explorer/components/AddStorageModal.tsx:405`). However, the story falls apart at the user experience: **there is no OAuth flow**. Users of Google Drive / OneDrive / Dropbox must paste a pre-obtained `access_token` + `refresh_token` themselves into a prompt. On top of that, `FileCopyJob` literally `panic!()`s on cloud paths (`core/src/ops/files/copy/job.rs:1042` and `:1485`), folder creation returns an error for cloud (`core/src/ops/files/create_folder/action.rs:119`), and cloud path resolution in the indexer is a `TODO` (`core/src/ops/indexing/path_resolver.rs:227`). Test coverage is three tests: one `#[ignore]`d live-S3 test, and two that use an in-memory OpenDAL operator (`core/tests/delete_strategy_test.rs:222` and `:250`). Methodology for the 55% figure: wiring layer (ops, CLI, UI, DB, encryption, `CloudBackend`) ≈ 85% done; indexing path exists but listing/read paths are implemented while write/copy/move/rename/change-detection are not; UX (OAuth, token refresh, validation, webhook change detection) ≈ 10% done. Averaging across user-facing capability surfaces, call it roughly half-built.

---

## 2. Git History Timeline

Authorship: **Jamie Pine** owns 22 of 25 HEAD-reachable cloud-file commits; Sinan Gencoglu (1, Dropbox OAuth config), slvnlrt (1, CodeRabbit review fixes), James Pine (1, UI). `git shortlog -sn HEAD -- <cloud files>`.

Chronological (HEAD-reachable only):

| Date | Commit | Author | Subject |
|---|---|---|---|
| 2025-10-13 | `ea5773a06` | Jamie Pine | Add Cloud variant to SdPath |
| 2025-10-13 | `c8e598b60` | Jamie Pine | Add Cloud variant to SdPath (re-apply) |
| 2025-10-13 | `e0076e56b` | Jamie Pine | Implement handling for cloud paths in file operations |
| 2025-10-13 | `861fc1e48` | Jamie Pine | Enhance volume backend integration for indexing operations |
| 2025-10-13 | `edf60394b` | Jamie Pine | Add cloud volume management operations |
| 2025-10-13 | `0208a67f8` | Jamie Pine | feat: implement cloud volume support |
| 2025-10-13 | `dc20ff0e6` | Jamie Pine | feat: add cloud module in CLI domain |
| 2025-10-13 | `91310317e` | Jamie Pine | feat: implement interactive cloud storage setup |
| 2025-10-14 | `56fcf9be2` | Jamie Pine | feat: add cloud volume handling |
| 2025-10-15 | `5eafd66bd` | Jamie Pine | feat: add cloud volume setup and migration changes |
| 2025-10-15 | `b26e2b5f5` | Jamie Pine | (epic): new SdPath addressing — LSYNC/FSYNC progress |
| 2025-10-16 | `79eb8d3c2` | Jamie Pine | feat: add cloud path skipping logic and cloud identifier support |
| 2025-11-17 | `c6cda350b` | Jamie Pine | feat: media proxy and thumbstrip functionality (added `is_cloud_path` in thumbnail job) |
| 2025-12-03 | `e5cb6baab` | Jamie Pine | feat: update cloud credential management |
| 2025-12-03 | `40d05fcec` | Jamie Pine | feat: add cloud credential entity and migration |
| 2025-12-03 | `0b22a7aec` | Jamie Pine | fix: update cloud credential manager instantiation |
| 2025-12-03 | `363bd39ff` | Jamie Pine | Add cloud path handling for thumbnail generation |
| 2025-12-18 | `cd000441f` | Jamie Pine | Update test configurations and enhance test robustness |
| 2025-12-24 | `06f0406b8` | Jamie Pine | feat: Implement file rename and folder creation (FILE-004) |
| 2026-01-03 | `68d6b36f4` | Sinan Gencoglu | Fix Dropbox OAuth configuration |
| 2026-01-09 | `90476fb81` | Jamie Pine | feat(types): enhance CloudStorageConfig with OAuth docs |
| 2026-03-15 | `1ff018454` | slvnlrt | fix: address CodeRabbit review feedback |
| 2026-04-14 | `37bb1b2f2` | James Pine | fix release CI + rustfmt + storage navigation |
| 2026-04-14 | `8a7fc53cd` | Jamie Pine | Fix ~750 TypeScript typecheck errors |

**Legacy cloud work (pre-October 2025)**: different architecture, now abandoned. The 2024 commits from Arnab, Ericson, Vítor, ameer2468 (`31f954f38`, `ab37341f7`, `eb098ebec`, `124fe5b0b`, `ee7044232`, `9d95ccb74`, etc.) reference `sd-cloud-schema`, iroh, a cloud ingester, and a "settings cloud page". A 2025-09-08 commit `e129a90e3` is titled "refactor: remove API design document and update cloud submodule" and by 2025-11-14 commit `f7d7468bc` "remove submodules" the `spacedrive-cloud` submodule is gone. The current cloud-drive work is a ground-up rewrite that sits on top of OpenDAL and the VDFS volume abstraction, not the old iroh-based "Cloud as a Peer" design.

No `git revert` commits target cloud functionality; the break is via submodule removal, not explicit reverts.

---

## 3. Active Work

**Branches**: Only `feature/cloud-drives-investigation` (the current branch) and `spacedrive-data` / `upstream/spacedrive-data` / `upstream/spacedrive-redundancy` are local. No active `cloud-*` or `oauth-*` feature branches. The investigation branch is 0 commits ahead of `upstream/main`.

**Task tracker**: `.tasks/core/CLOUD-003-cloud-volume.md` is marked `status: In Progress`, `assignee: jamiepine`, `last_updated: 2025-10-14`. `.tasks/core/FILE-003-cloud-volume-file-operations.md` is `status: To Do`. The parent epic `.tasks/core/CLOUD-000-cloud-as-a-peer.md` is `To Do` — note that this epic is about "Cloud as a Peer" (managed sd-core in the cloud via iroh/relay), not about OpenDAL cloud storage, so it's a different line of work.

**Recent activity on HEAD**: the last substantive cloud commit on HEAD is `90476fb81` (2026-01-09 — comment-only OAuth documentation). Since then, only CI/typecheck/UI-rename sweeps (`8a7fc53cd`, `37bb1b2f2`) have touched cloud code. Cloud features have been effectively frozen for ~3 months.

---

## 4. Test Coverage Report

Total tests touching cloud code: **5**.

| File | Test | Type | Status |
|---|---|---|---|
| `core/src/volume/backend/cloud.rs:452` | `test_cloud_backend_s3` | Integration (requires real S3 env vars) | `#[ignore]` |
| `core/src/crypto/cloud_credentials.rs:357` | `test_encrypt_decrypt_credential` | Unit | Runs |
| `core/src/crypto/cloud_credentials.rs:427` | `test_credential_expiry` | Unit | Runs |
| `core/tests/delete_strategy_test.rs:221` | `test_cloud_backend_delete_file` | Integration (OpenDAL `services::Memory`) | Runs |
| `core/tests/delete_strategy_test.rs:249` | `test_cloud_backend_delete_directory` | Integration (OpenDAL `services::Memory`) | Runs |

**What's tested**: credential encrypt/decrypt round-trip through `CloudCredentialManager`, credential expiry check, generic `VolumeBackend` delete semantics on a memory operator, credential DB insertion.

**What's NOT tested**:
- S3 / Google Drive / OneDrive / Dropbox / Azure / GCS builders (real or mocked)
- OAuth token refresh behavior (not implemented anyway)
- `VolumeAddCloudAction` (no unit tests; no tests calling `execute()`)
- `VolumeRemoveCloudAction`
- Volume manager rehydration of cloud volumes from DB (`core/src/volume/manager.rs:198`)
- Indexing against a cloud mount point
- Thumbnail generation against `is_cloud_path` branch (`core/src/ops/media/thumbnail/job.rs:604`)
- Location validation against a cloud volume (`core/src/location/manager.rs:550`)
- Cloud path resolution in the indexer (`core/src/ops/indexing/path_resolver.rs:226`)
- File copy / move / rename / folder-create for cloud paths (and for good reason — they `panic!`)

No `.github/workflows/*.yml` references cloud, so there is **no CI job** gating cloud behavior. `rtk git grep -lEi "cloud" -- ".github/workflows/"` returned nothing.

---

## 5. Placeholder Inventory

Aggregated TODO / FIXME / unimplemented / "not yet" markers touching cloud code across `core/src/`:

| File | Line | Marker | Content |
|---|---|---|---|
| `core/src/ops/files/copy/job.rs` | 1042 | `panic!` | `SdPath::Cloud { .. } => panic!("Cloud storage operations are not yet implemented")` |
| `core/src/ops/files/copy/job.rs` | 1485 | `panic!` | Same, in second branch |
| `core/src/ops/files/create_folder/action.rs` | 119 | Error return | `"Cloud folder creation not yet implemented"` |
| `core/src/ops/indexing/path_resolver.rs` | 227 | `TODO` | `// TODO: Implement cloud path resolution` (returns `Ok(None)`) |
| `core/src/ops/locations/add/action.rs` | 198 | `TODO` | `// TODO: Validate that the path exists on the cloud volume` |
| `core/src/ops/locations/add/action.rs` | 217 | `TODO` | `// TODO: Implement proper duplicate detection for both Physical and Cloud paths` |
| `core/src/location/manager.rs` | 570 | `TODO` | `// TODO: Validate that we can connect to the volume` in `validate_cloud_path` |
| `core/src/volume/backend/cloud.rs` | 368 | inline note | `// Most cloud services don't provide creation time` (correct, not a stub) |
| `core/src/volume/backend/cloud.rs` | 336 | inline note | `// Cloud storage doesn't have inodes` (correct, not a stub) |

**Zero `todo!()` / `unimplemented!()` macros** exist inside cloud modules themselves. The two `panic!` calls are the most serious — a user hitting file-copy against any cloud path crashes the daemon job.

Documentation has its own "Known Issues" block (`docs/core/cloud-integration.mdx:368`) listing three issues honestly: no change detection, no file watcher, no OAuth token refresh.

---

## 6. Wiring Status

**Can a user connect a Google Drive account via CLI or UI right now?**

**CLI path** (`sd-cli cloud`):
1. `Commands::Cloud => cloud::run(&ctx)` — wired (`apps/cli/src/main.rs:714`).
2. Interactive menu offers S3 / Google Drive / OneDrive / Dropbox / Azure / GCS (`apps/cli/src/domains/cloud/setup.rs:53`).
3. For Google Drive, prompts: client_id, client_secret, **access_token**, **refresh_token** (`setup.rs:158-163`). **Breakage point: the user must obtain tokens themselves**. The CLI points them to `https://console.cloud.google.com/apis/credentials` (`setup.rs:156`) and says "After authorizing, you'll receive tokens" — but Spacedrive does nothing to perform the OAuth dance. There is no redirect server, no browser launch, no `/oauth/callback` endpoint. Searching `core/src/` and `apps/cli/` for `authorize|redirect|callback|localhost.*oauth` returns zero matches.
4. Assuming the user somehow has valid tokens, `VolumeAddCloudAction::execute` (`core/src/ops/volumes/add_cloud/action.rs:95`) instantiates `CloudBackend::new_google_drive` (`cloud.rs:77`), stores an encrypted `CloudCredential::new_oauth` via `CloudCredentialManager::store_credential` (`cloud_credentials.rs:59`), registers the volume with the manager (`action.rs:480`), and calls `track_volume` (`action.rs:484`). This part works.
5. User is then told to `sd location add` to attach a location (`setup.rs:359`). Location add will call `validate_cloud_path` which only checks DB existence of the volume — it does not actually hit the cloud (`location/manager.rs:570` TODO).
6. Indexing the new cloud location: the indexer branches on `cloud_path()` (`core/src/ops/indexing/job.rs:250`) and skips certain logic for cloud paths (commit `79eb8d3c2`). Directory listing via `CloudBackend::read_dir` (`cloud.rs:295`) is implemented. So metadata-level indexing should work.
7. Copy a file to this cloud volume: **daemon panics** (`copy/job.rs:1042`).
8. Create a folder: error "Cloud folder creation not yet implemented" (`create_folder/action.rs:119`).
9. Thumbnail generation from cloud: implemented via download-to-tempfile path (`thumbnail/job.rs:608`).

**Tauri/Web UI path** (`AddStorageModal.tsx`):
1. Full modal exists with icons for all 12 providers (`AddStorageModal.tsx:139-212`).
2. Calls `useLibraryMutation("volumes.add_cloud")` (`AddStorageModal.tsx:405`).
3. For OAuth providers, renders plain `Input` fields for `access_token` and `refresh_token` (`AddStorageModal.tsx:1371`, `:1379`). Same UX gap as CLI: user must paste tokens obtained elsewhere.

**Daemon startup wiring**: `VolumeManager` rehydrates cloud volumes from the DB by calling each `CloudBackend::new_*` with decrypted credentials (`core/src/volume/manager.rs:198-339`). This closes the loop between "user adds cloud volume" and "daemon restart restores the operator".

**Verdict**: end-to-end "add a bucket, browse it, see thumbnails" works for S3-style providers if the user supplies raw access keys. For OAuth providers, the workflow is theoretically complete but practically unusable outside of developer testing.

---

## 7. Feature Flags & Build

There are **no Rust feature flags** gating cloud code. `grep -n "cfg(feature" core/Cargo.toml` returns nothing cloud-related. The `opendal = "0.54"` dependency is unconditional with features `services-s3, services-gdrive, services-onedrive, services-dropbox, services-azblob, services-gcs` (`core/Cargo.toml:88-95`). Note that `services-memory` is NOT listed in core's manifest but the integration tests use it, implying it comes transitively from opendal default features. Also: pCloud, Backblaze B2, Wasabi, and DigitalOcean Spaces are in the `CloudServiceType` enum (`volume/backend/mod.rs:84-89`), but **the first three lack a dedicated `CloudBackend::new_*` constructor** — B2/Wasabi/Spaces are routed through `new_s3` in the CLI (`setup.rs:87-95`) and have no opendal feature flag of their own.

Cloud code is compiled into every build: default features, no conditional cfgs, no workspace profile gating. A `cargo build` always includes it.

---

## 8. Gap Matrix

| Capability | Docs say | Code has | Tests | Status |
|---|---|---|---|---|
| S3-compatible bucket via access key | Supported (`cloud-integration.mdx:20-29`) | `CloudBackend::new_s3` (`cloud.rs:48`) + action | Memory-backend integration tests only | **Works** (untested against real S3) |
| Google Drive via OAuth | Supported (`cloud-integration.mdx:46`) | Backend + action; no OAuth flow | None | **Partial — user must supply tokens manually** |
| OneDrive via OAuth | Supported | Same as Google Drive | None | **Partial — same gap** |
| Dropbox via OAuth | Supported | Same; `refresh_token` only (opendal auto-refreshes) | None | **Partial — same gap** |
| Azure Blob | Supported | `new_azure_blob` (`cloud.rs:172`) | None | **Works in theory** |
| Google Cloud Storage | Supported | `new_google_cloud_storage` (`cloud.rs:201`) | None | **Works in theory** |
| Backblaze B2, Wasabi, DO Spaces | Listed in enum | Routed through S3 builder | None | **Works as S3-compatible** |
| pCloud, iCloud, MEGA, SharePoint, Box, Nextcloud, Seafile, WebDAV, Alibaba OSS, Tencent COS, Huawei OBS, Baidu BOS | "Supported" (`cloud-integration.mdx:82-90`) | Not implemented | None | **Doc lies** — only 6 backends exist, enum covers 9 |
| Encrypted credential storage | Supported (`cloud-integration.mdx:115`) | `CloudCredentialManager` with XChaCha20-Poly1305 (`cloud_credentials.rs:194`) | Unit tests cover round-trip | **Works** |
| OS keyring storage | Claimed (`cloud-integration.mdx:117`) | **NOT implemented** — credentials are encrypted and stored in the SQLite library DB (`cloud_credentials.rs:78-113`), not the OS keyring | N/A | **Doc lies** |
| Content-identity hashing on cloud (58KB sample) | Claimed (`cloud-integration.mdx:96`) | `read_range` exists on `CloudBackend` (`cloud.rs:266`); indexer wiring unverified in this audit | None | **Unclear** |
| Indexing cloud directory | Supported | `read_dir` works; `path_resolver::resolve_to_entry` is TODO for cloud (`path_resolver.rs:226`) | None | **Partial** |
| Thumbnails from cloud files | Supported | Download-to-temp path (`thumbnail/job.rs:604`) | None | **Implemented** |
| File copy to/from cloud | Supported (`cloud-integration.mdx:167`) | `panic!` in `FileCopyJob` (`copy/job.rs:1042`, `:1485`) | None | **Broken — doc lies** |
| Folder create on cloud | Implicit | Returns error (`create_folder/action.rs:119`) | None | **Not implemented** |
| File rename on cloud | Implicit | Not covered — `FILE-004` rename didn't include cloud | None | **Not implemented** |
| Change detection on cloud | Doc notes "not yet implemented" (`cloud-integration.mdx:370`) | Cloud entries treated as new each reindex | None | **Not implemented — doc is honest** |
| Real-time file watcher for cloud | Doc notes "does not support" (`cloud-integration.mdx:372`) | Not implemented | N/A | **Not planned** |
| OAuth token refresh | Doc notes "not yet implemented" (`cloud-integration.mdx:374`) | Opendal's Dropbox client auto-refreshes, but Spacedrive does not persist rotated tokens back to the encrypted store | N/A | **Partial** |
| Browser-based OAuth flow | NOT documented | NOT implemented | N/A | **Missing** |
| Remove cloud volume | Implicit | `VolumeRemoveCloudAction` (`remove_cloud/action.rs:32`) | None | **Works** |
| Multi-cloud search | Claimed (`cloud-integration.mdx:300-310`) | Depends on indexing resolution (TODO at `path_resolver.rs:227`) | None | **Doc overstates** |
| Cross-cloud move | Claimed (`cloud-integration.mdx:189-206`) | `panic!` | None | **Broken — doc lies** |
| CI coverage | — | No workflow touches cloud | N/A | **Missing** |

---

## 9. Honest Verdict

Cloud drives in Spacedrive are **functional scaffolding with broken glass at the edges**. The skeleton is real and well-integrated: OpenDAL 0.54 is unconditionally linked, a 485-line `CloudBackend` implements the `VolumeBackend` trait, the `volumes.add_cloud` / `volumes.remove_cloud` actions are registered in the inventory registry, credentials are encrypted with XChaCha20-Poly1305, there's a SeaORM entity with its own migration, the volume manager rehydrates cloud volumes on startup, the CLI has a dedicated interactive domain, and the React UI has a 1,488-line modal. This isn't a greenfield commit — it's two months of real engineering (Oct 13–Dec 24, 2025) by a single developer.

But a user cannot sit down today and connect their Google Drive. They'd need to go to Google Cloud Console, create an OAuth app, manually run a token exchange against Google's OAuth endpoints (using curl or a separate tool), then paste `access_token` + `refresh_token` into a prompt. The product assumes a developer, not an end user. And even after connecting, any attempt to copy a file to or from that cloud volume `panic!`s the daemon, and trying to create a folder returns an error. The docs promise features like "cross-cloud move" (`cloud-integration.mdx:189`), "multi-cloud search" (`:300`), and "OS keyring" credential storage (`:117`) that are not in the code.

Call it **~55% complete toward an MVP**: the hard infrastructure (backend, encryption, DB, IPC, UI) is done, but the UX-critical pieces (OAuth flow, write-path file ops, change detection) and the test coverage required to trust any of it are not. There has been no meaningful cloud commit since January 2026; the work is effectively parked behind other priorities (volume/redundancy/Tailscale per late-March/April 2026 commits).

If someone asks "can I connect a Google Drive to Spacedrive today," the honest answer is "only if you're willing to paste raw OAuth tokens into a CLI prompt, and only for read/browse — writes will crash the daemon."
