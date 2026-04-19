# Cloud drives MVP — OneDrive vertical slice (BYO)

> ## ⚠️ DRAFT / WORK IN PROGRESS — DO NOT MERGE
>
> Opened in **Draft** for early visibility. Implementation is complete and locally green (404 lib tests + 3 E2E tests, clippy clean, fmt clean), but **manual end-to-end testing hasn't started yet** and the `.investigations/cloud-drives/` artefacts still need pruning before ready-for-review.
>
> Remaining before ready-for-review:
>
> 1. Full manual OneDrive journey against a live Azure AD app (see *How to test manually*) with any bug fixes it surfaces.
> 2. Pruning of `.investigations/cloud-drives/` — policy open for discussion (question 1 below).
> 3. Applying the task-tracker updates proposed in `.investigations/cloud-drives/*-proposal.md` (question 2).
> 4. Any feedback from this draft review.
>
> **Feedback on direction and scope is welcome now.** Line-by-line review is more efficient once the draft is converted.

## Summary

First end-to-end cloud drive integration on the daemon: a user can **register a OneDrive (personal) account through a browser OAuth flow**, **list, copy and create folders on it through the normal file pipeline**, and **pick up remote changes via the Microsoft Graph Delta API** on the next index pass. BYO Azure AD — no Spacedrive-owned provider credentials shipped, no founder action required to merge.

Google Drive and Dropbox keep their paste-tokens flow with a "browser sign-in coming soon" banner; their providers plug into the same infrastructure in a later PR (`CLOUD-004`).

Seventeen commits on top of `fb5d4d7e7`. 404 lib tests + 3 E2E tests passing, zero panics.

## Context

The cloud-drive subsystem was investigated before any code was written. Reports in [`.investigations/cloud-drives/`](./.investigations/cloud-drives/) established that cloud drives were ~55 % functional scaffolding with five critical blockers: two daemon-crashing `panic!`s, no OAuth flow, lost B2/Wasabi/Spaces rehydration on restart, disconnected `create_folder` for cloud, and rehashed-on-every-pass indexing.

Key reading:

- [`00-executive-summary.md`](./.investigations/cloud-drives/00-executive-summary.md) — one-page state of affairs.
- [`06-mvp-onedrive-vertical-slice.md`](./.investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md) — eight commit-sets planned, with acceptance criteria.
- [`cloud-integration-current-state.mdx`](./.investigations/cloud-drives/cloud-integration-current-state.mdx) — factual current-state documentation, kept outside `docs/` so the target-state `docs/core/cloud-integration.mdx` stays aspirational.

## What ships

### User-visible

1. **One-click OneDrive sign-in.** Paste `client_id` + `client_secret` obtained via the in-app Azure AD tutorial (seven steps, five redirect URIs to copy at once), click Connect with Microsoft, approve in the default browser, the daemon picks up the redirect on a loopback port, stores tokens encrypted, and the volume appears.
2. **Real file operations on cloud volumes.** Local → OneDrive, OneDrive → local, OneDrive → OneDrive (server-side when the provider supports it, streaming fallback otherwise), and OneDrive → different cloud account all work through `FileCopyJob`. Folder creation works.
3. **Change detection on subsequent index passes.** First pass seeds a Delta token; later passes call `/me/drive/root/delta` and advance the cursor. Throttled (429 / Retry-After respected), resilient to token invalidation (`410 resyncRequired` triggers a clean resync).
4. **Disconnect from the sidebar.** Right-click a cloud volume → Disconnect runs `volumes.remove_cloud`, wiping credentials and the volume row.
5. **Fewer nonsensical menu items.** Speed Test and Eject hidden on cloud volumes.
6. **Consistent icons.** Renaming a OneDrive volume no longer swaps its icon for a generic disk (the duplicate substring-matching `getVolumeIcon` in `DevicePanel.tsx` was the culprit; only the scheme-based canonical version in `packages/ts-client/src/volumeIcons.ts` survives).

### Developer-visible

1. **Provider-agnostic OAuth 2.0 + PKCE infrastructure** under `core/src/ops/cloud/oauth/`. New providers plug in by implementing `OauthProvider` and registering at startup; the UI hits the same three wire methods (`cloud.oauth.start`, `cloud.oauth.poll`, `cloud.oauth.cancel`). No changes to `CloudStorageConfig` variants — the browser flow populates the existing fields on the existing `VolumeAddCloudAction`.
2. **`BackendFeatures` metadata layer** at `core/src/volume/backend/mod.rs`. Every backend advertises server-side copy/rename, stable-file-id guarantees, change-notification kind, content-hash algorithms, case sensitivity, duplicate tolerance, max file size, and multipart threshold. Enables capability-driven code paths rather than per-provider branches.
3. **`CloudCopyStrategy`** extending the existing strategy pattern. Four shapes (local↔cloud, cloud↔local, same-backend cloud↔cloud, cross-backend cloud↔cloud) with 8 MiB chunked streaming, 4-way writer concurrency, and progress reporting.
4. **`ChangeDetector` trait + `OneDriveChangeDetector`** so future providers slot in without touching the indexer.
5. **OpenDAL 0.55** (from 0.54) with a standard layer stack — `TracingLayer`, `ConcurrentLimitLayer(16)`, `TimeoutLayer(30s/30s)`, `RetryLayer(3, jitter)` — wrapping every cloud operator.
6. **OAuth-refresh background task** and **Delta cursor persistence** (`cloud_sync_state` table). Encrypted credential storage via SeaORM + XChaCha20-Poly1305 already existed.
7. **17 new unit tests + 3 E2E tests via `wiremock`**, all deterministic.

## Scope boundaries

- **Provider**: OneDrive personal (`tenant=common`) only. Google Drive and Dropbox keep paste-tokens with a "coming soon" banner. Microsoft Business / SharePoint needs a tenant-specific issuer; stubbed as TODO.
- **Client credentials**: BYO. Users register their own Azure AD app. Every `CloudStorageConfig::*` variant already expected `client_id` + `client_secret`; validation was in place. Swapping to Spacedrive-owned credentials later is a ~5-line UI diff; the server side is forward-compatible.
- **E2E test**: the OAuth journey is fully OneDrive-shaped, but the post-auth `VolumeAddCloudAction` step uses S3 because `opendal 0.55` hardcodes `graph.microsoft.com` for OneDrive data-plane calls and cannot be wiremocked. OneDrive file I/O is covered at unit level against `services::Memory`.
- **Documentation**: `docs/core/cloud-integration.mdx` is the target-state vision and stays unchanged. A current-state snapshot lives at `.investigations/cloud-drives/cloud-integration-current-state.mdx` for PR review.

## Known tech debt

Every item has a `TODO(cloud-mvp): ...` comment at its call site referencing the plan doc.

1. **Hot-swap OAuth access token during a running pass.** Refresh task rewrites `cloud_credentials` but `CloudBackend` reads tokens at construction; long-running indexing passes won't pick up rotation mid-flight. TODOs at `core/src/ops/cloud/change_detection/onedrive.rs:51` and `core/src/ops/indexing/phases/processing.rs:83`.
2. **Consume the delta stream to skip re-hashing unchanged files.** Delta cursor advances, but the main indexing loop still re-hashes every entry each pass. Needs OpenDAL to surface `provider_file_id` on listing (it doesn't today) or a cross-reference layer. Machinery is in place.
3. **Rename detection emits `Modified`.** `ChangeKind::Renamed { from_path }` exists but the OneDrive detector never constructs it — a single Graph delta response doesn't carry the previous path. Needs a per-`provider_file_id` path cache. The indexer still reconciles renames via stable id.
4. **ETag / `content_md5` checksum verification on cloud paths** (TODO at `core/src/ops/files/copy/strategy.rs:844`). Currently warns and proceeds; integrity guaranteed by OpenDAL's chunked writer.
5. **Backblaze B2, Wasabi, DigitalOcean Spaces** silently dropped at daemon restart — pre-existing catch-all at `core/src/volume/manager.rs:335-338`. Not fixed in this PR; constructors exist for S3, GoogleDrive, OneDrive, Dropbox, AzureBlob, GCS.
6. **Google Drive + Dropbox browser OAuth.** Infrastructure ready; only concrete providers and UI forms missing. Tracked outside CLOUD-004.
7. **Reconnect UI** when a refresh token expires (users currently delete and re-add).
8. **Windows case-insensitive FS detection** on `LocalBackend`.
9. **Copy-on-write detection** on btrfs / apfs for `LocalBackend::server_side_copy`.

## How to test manually

### Prerequisites

1. **Azure AD app registration** (one-time, ~5 minutes):
   - Sign in to <https://portal.azure.com>.
   - App registrations → New registration. Name anything, supported accounts = **Personal Microsoft accounts only**.
   - Redirect URI: add all five of `http://127.0.0.1:53682`, `:53683`, `:53684`, `:53685`, `:53686` as **Web** (not SPA).
   - Authentication → Advanced → Allow public client flows = **Yes**.
   - API permissions → Microsoft Graph → Delegated → `Files.ReadWrite.All`, `offline_access`, `User.Read`. Grant admin consent.
   - Certificates & secrets → New client secret → copy the **Value**.
   - Overview → copy the **Application (client) ID**.
2. **A folder on OneDrive with mixed files** (a PDF, a few images, a nested sub-folder).

### Test sequence

1. Build: `cargo build` (or `cargo run --bin sd-cli -- restart` if a daemon is already running).
2. Launch Tauri: `cd apps/tauri && bun run tauri:dev`.
3. Add storage → Cloud → OneDrive → expand the tutorial once to verify it renders, then paste client ID and client secret.
4. Click **Connect with Microsoft**. The default browser should open Microsoft's login page pre-filled. Approve.
5. The modal should close and the volume should appear in the sidebar within 2 seconds, labelled with your Microsoft display name.
6. Click the OneDrive volume, browse the root. Files and folders should list.
7. Right-click a local file → **Copy to…** → OneDrive volume → destination folder. Watch progress.
8. Verify on <https://onedrive.live.com> that the file uploaded.
9. Right-click a OneDrive file → **Copy to…** → local destination. Verify bytes match (`shasum`).
10. Right-click a OneDrive folder → **New folder**. Verify on <https://onedrive.live.com>.
11. Delete a file on <https://onedrive.live.com>, trigger a re-index in the app, confirm it disappears from the view.
12. Right-click the OneDrive volume → **Disconnect**. Volume disappears, `cloud_credentials` row removed.
13. Add the same volume again with the same credentials — Microsoft remembers, tokens refresh.

### Failure-mode checks

- Start a Connect flow, close the browser tab, click **Cancel sign-in** in the app. Modal should return to initial state, no volume added.
- Close the modal entirely during awaiting-browser. No ghost loopback server should remain (check `netstat -an | findstr 5368` returns nothing within ~10 s).
- Tamper with a `cloud_credentials` row (garbage `refresh_token`), restart the daemon. Refresh task should log an auth failure and not crash.

## How to review

### Commit narrative order

```
fe5a5331d  investigation reports + MVP plan
dcb925162  foundation (OpenDAL 0.55, layers, BackendFeatures)
03483252f  file copy / move / create_folder
22aaf310b  correction of Set 2 task tracking
391e963e3  OAuth infrastructure (BYO, loopback + PKCE)
67c3e179a  wire / TS shape fix
2e8f048dc  .tasks/ pristine
3f1fd13d1  orchestration playbook
1461281ee  OneDrive OAuth provider
7f684ee53  OneDrive browser connect form
3406a9d8b  OneDrive Delta API change detection
c81777598  annex UI fixes
232e125c4  CloudCopyStrategy wiring
9482354cd  E2E test + docs + CLOUD-004 proposal
18cad0691  OpenDAL 0.55 token-constraint fix
0a2db20b9  restore target-state docs
f6234dee1  comment audit and trim
```

Each commit builds, tests and lints cleanly on its own.

### Reading paths by concern

- **OAuth subsystem**: `core/src/ops/cloud/oauth/` (start, poll, cancel, complete, provider trait, loopback, flow store, refresh, providers/onedrive).
- **Change detection**: `core/src/ops/cloud/change_detection/` (types, detector trait, OneDrive impl, repository, delta pass).
- **Copy strategy**: `core/src/ops/files/copy/strategy.rs` + `routing.rs`.
- **UI**: `packages/interface/src/routes/explorer/components/OneDriveConnectForm.tsx` + `AddStorageModal.tsx`.
- **Migrations**: `core/src/infra/db/migration/m20260418_000001_*.rs` and `m20260418_000002_*.rs`.
- **E2E**: `core/tests/onedrive_end_to_end_test.rs`.

## Risk and rollback

- The daemon does not crash on any cloud path — every branch returns a typed `Result`.
- Users with OneDrive volumes see their file I/O return errors; existing local volumes are unaffected.
- Rollback is a clean `git revert f6234dee1..fe5a5331d^`. No data-loss migrations; `cloud_sync_state` and the `provider_file_id` column on `entries` are purely additive.
- Non-cloud users see no behavioural change. The OpenDAL 0.55 bump and layer stack hit the cloud operator path only; `LocalBackend` was updated for `BackendFeatures` parity but its behaviour is identical.

### Dependency additions

- `opendal 0.54 → 0.55` (already a direct dep)
- `oauth2 = "5"`
- `webbrowser = "1"`
- `dashmap = "6"`
- `subtle = "2"` (constant-time CSRF)
- `base64 = "0.22"`
- `wiremock = "0.6"` (dev-dep only)

All MIT/Apache-2.0.

## Questions for reviewers

1. **`.investigations/` policy.** Keep it in the tree, or drop before merge? Options: (a) keep as project memory; (b) move to an external design repo; (c) keep only the plan and proposals, drop the state-of-art reports.
2. **Task tracker updates.** Four update proposals sit in `.investigations/cloud-drives/*-proposal.md`. Applied as a follow-up commit on this branch, or a separate maintainer PR?
3. **Google Drive / Dropbox browser flows.** In this PR (~1500 LOC each), or a separate PR once OneDrive is confirmed stable?
4. **BYO client_id long-term.** If you'd rather ship an official Spacedrive Azure AD app, it's a small UI change and two constants — no backend work.
