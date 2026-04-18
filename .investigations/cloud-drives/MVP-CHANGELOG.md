# OneDrive MVP — Changelog draft

This file records every user-visible and developer-visible change landed on
`feature/cloud-drives-investigation` for the OneDrive MVP, ordered
chronologically by commit. It is ready to be merged into an upstream
`CHANGELOG.md` when the founder introduces one; no top-level changelog
exists in the repository today.

Branch: `feature/cloud-drives-investigation`
Baseline: `fb5d4d7e7`
Tip at time of writing: Set 8b pending; Set 8a is `232e125c4`.

## OpenDAL foundation (Set 1)

- Bumped `opendal` `0.54` → `0.55` and adapted the two `jiff::Timestamp`
  conversion sites in `core/src/volume/backend/cloud.rs`.
- Wrapped every `opendal::Operator` in a consistent layer stack: `TracingLayer`
  innermost, `ConcurrentLimitLayer(16)`, `TimeoutLayer(30s / 30s io)`,
  `RetryLayer(3 retries, jitter)` outermost.
- Introduced `BackendFeatures` describing server-side copy / rename, stable
  file ids, change-notification kind, content hash support, case sensitivity,
  duplicate tolerance, max file size, and multipart threshold. Populated for
  `LocalBackend` and `CloudBackend` per service type.
- Enriched `RawMetadata` with `etag`, `version`, `content_md5`, and
  `provider_file_id`.
- Fixed a latent bug where `CloudBackend::exists()` swallowed auth failures as
  `false`; it now distinguishes `NotFound` (→ `false`) from every other error
  (→ `Err`).

## File copy and folder creation (Set 2)

- Replaced the two `panic!` branches in `core/src/ops/files/copy/job.rs` with
  typed errors; the daemon no longer crashes when a cloud endpoint is
  involved in a copy.
- Wired `CreateFolderAction` to `CloudBackend::create_directory`; the cloud
  branch was previously hardcoded to return `ActionError::Internal("not yet
  implemented")`.
- Extended `FileCopyError` with cloud-specific variants.

## OAuth infrastructure — BYO (Set 3)

- Added `oauth2`, `webbrowser`, `dashmap`, `subtle`, `base64` dependencies.
- New subsystem at `core/src/ops/cloud/oauth/` providing a provider-agnostic
  OAuth 2.0 PKCE + loopback authorization-code flow.
- `OauthProvider` trait + `OauthProviderRegistry` (starts empty).
- `OauthFlowStore` state machine with `Pending` / `Completed` / `Failed` /
  `Cancelled` statuses, backed by `DashMap` with per-flow cancellation
  watches.
- Janitor evicts pending flows after 10 minutes and terminal flows after a
  30-second grace.
- One-shot loopback HTTP server with constant-time CSRF state validation
  (`subtle::ConstantTimeEq`), minimal success HTML, 5-minute timeout,
  cancellation-aware.
- Three registered library operations: `cloud.oauth.start`,
  `cloud.oauth.poll`, `cloud.oauth.cancel`.
- `CloudTokenRefreshTask` — daemon-side background task rotating OAuth
  access tokens 5 minutes before expiry across every library, preserving
  rotated refresh tokens when the provider rotates.
- Corrected a wire / TypeScript mismatch: `OauthFlowStatus::Completed`
  nests `tokens` rather than flattening (specta does not honour
  `#[serde(flatten)]` on enum variants).

## OneDrive OAuth provider (Set 4)

- `OneDriveProvider` targeting the Microsoft identity platform `tenant=common`
  endpoint for personal accounts.
- Scopes: `Files.ReadWrite.All offline_access User.Read`.
- Loopback ports `53682..=53686` — Microsoft requires exact-match redirect
  URIs; the app registration must list all five, and the first free port
  wins at sign-in time.
- PKCE S256 challenge, `prompt=select_account` so users with multiple
  Microsoft identities always see the account chooser.
- Graph `/me` lookup hydrates a display name for the volume label, with
  `userPrincipalName` fallback.
- Microsoft's refresh-token rotation handled idempotently — if the response
  omits a fresh refresh token, the prior one is preserved.

## OneDrive browser sign-in UI (Set 5)

- `AddStorageModal` OneDrive section replaced with a dedicated
  `OneDriveConnectForm` carrying a collapsible 7-step BYO Azure AD tutorial,
  a "copy redirect URIs" affordance, and `clientId` / `clientSecret` inputs.
- "Connect with Microsoft" button launches `cloud.oauth.start`, opens the
  returned `auth_url` via `openExternal`, and polls `cloud.oauth.poll` every
  second until the flow reaches a terminal state.
- On `Completed`, the UI calls `volumes.add_cloud` with the returned tokens
  and the display-name label; on `Failed` a toast surfaces the provider
  message; on `Cancelled` the UI clears state.
- Google Drive and Dropbox modals keep their paste-tokens UI with a banner
  reading "Browser sign-in coming soon for Google Drive and Dropbox".

## Change detection — OneDrive Delta API (Set 6)

- New `cloud_sync_state` table (`volume_id`, `provider`, `change_token`,
  `last_full_sync_at`, `last_incremental_at`, `consecutive_failures`,
  timestamps) with a SeaORM entity and a `CloudSyncStateRepository` trait +
  `SeaOrmCloudSyncStateRepository` implementation.
- `ChangeDetector` trait with `initial_token` and `changes_since` methods
  returning a paginated `ChangesPage` that distinguishes intra-scan
  pagination (`next_token`) from end-of-changes (`end_token`).
- `OneDriveChangeDetector` over Microsoft Graph `/me/drive/root/delta`,
  mapping HTTP 410 `resyncRequired` → `Invalidated`, 429/503 →
  `RateLimited { retry_after_secs }`, 401/403 → `Auth`.
- Per-page `change_token` checkpointing so interrupted syncs resume from the
  last good page.

## Annex UI fixes (Set 7)

- Right-click "Disconnect" item on cloud volumes now fires
  `volumes.remove_cloud`; "Speed Test" and "Eject" are hidden on cloud
  volumes (they do not apply).
- `GroupType::Cloud` hidden from the Add Group and Space Customization
  dropdowns until the corresponding renderer lands; the Rust variant is
  retained so existing JSON rows still deserialize.
- Unified `getVolumeIcon` — removed the weaker substring-match fallback in
  `routes/overview/DevicePanel.tsx` and canonicalized on the parsing version
  in `packages/ts-client/src/volumeIcons.ts`.

## CloudCopyStrategy wiring (Set 8a)

- `CloudCopyStrategy` dispatches four copy shapes:
  - Same-backend server-side copy with a streaming fallback when the
    provider does not advertise `write_can_copy`.
  - Local → cloud streaming upload.
  - Cloud → local streaming download.
  - Cross-backend cloud → cloud streaming.
- `CopyStrategyRouter` detects cloud endpoints and routes accordingly; a
  local-local call that somehow reaches `CloudCopyStrategy` surfaces as a
  typed error identifying the routing bug rather than panicking.
- Checksum verification on cloud paths emits a `warn!` and proceeds until
  the ETag / `content_md5` plumbing lands.

## E2E test, docs, proposals (Set 8b)

- `core/tests/onedrive_end_to_end_test.rs`: three `#[tokio::test]` cases
  against wiremock covering the full OAuth journey (start → browser
  callback → poll → Completed → `volumes.add_cloud` →
  `volumes.remove_cloud`), the cancel path, and the token-exchange failure
  path.
- Complete rewrite of `docs/core/cloud-integration.mdx` to reflect what the
  MVP actually ships — replacing the aspirational claims (40+ providers,
  OS keyring storage, cross-cloud moves, thumbnail caching, cost tracking)
  with a real supported-services matrix, BYO Azure AD tutorial, security
  model, and an honest "Known limitations" section.
- Finalised `.investigations/cloud-drives/CLOUD-004-oauth-infrastructure-proposal.md`
  so the founder can copy it into `.tasks/core/` when reviewing the PR.
- Sibling proposals at `.investigations/cloud-drives/CLOUD-003-update-proposal.md`
  and `.investigations/cloud-drives/FILE-003-update-proposal.md` summarising
  the acceptance-criteria status deltas for those tasks.

## Known carry-over tech debt

Tracked in each of the three proposals above:

- Hot-swap `CloudBackend` on token refresh (refresh task rewrites the
  credential row but `VolumeManager` still holds the stale token until
  reload).
- Delta stream not yet consumed to skip re-hashing unchanged cloud entries
  during indexing.
- Backblaze B2, Wasabi, DigitalOcean Spaces silently dropped at daemon
  restart by a catch-all in `VolumeManager::restore_cloud_volumes`.
- Google Drive and Dropbox paste-tokens flows still in place; browser
  sign-in roll-out tracked outside CLOUD-004.
- OneDrive Business / SharePoint tenants require a tenant-specific issuer;
  MVP is `tenant=common` only.
- No reconnect UI when the refresh task gives up on a credential; users
  today must delete and re-add the volume.
- ETag / `content_md5` checksum verification for cloud copies deferred.
- Cross-backend server-side copy streams through the daemon (S3 bucket A →
  S3 bucket B cross-account never uses provider-side copy).
