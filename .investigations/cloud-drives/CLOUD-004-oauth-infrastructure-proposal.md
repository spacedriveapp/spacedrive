---
id: CLOUD-004
title: Cloud OAuth Infrastructure (BYO) and OneDrive Vertical Slice
status: Done
assignee: jamiepine
parent: CLOUD-003
priority: High
tags: [cloud, oauth, security, onedrive]
whitepaper: Section 5.2
related_tasks: [CLOUD-003, FILE-003]
last_updated: 2026-04-18
---

## Description

Provider-agnostic OAuth 2.0 infrastructure and a complete OneDrive vertical slice: browser sign-in, volume registration, change detection, and cleanup. Architecture is strictly Bring-Your-Own — the user supplies `client_id` and `client_secret` from an Azure AD app they register themselves. Spacedrive ships no hardcoded OAuth clients.

This proposal is ready for the founder to copy into `.tasks/core/CLOUD-004-oauth-infrastructure.md` when reviewing the MVP PR.

## Delivered (Set 3 → Set 8b)

- [x] OAuth subsystem at `core/src/ops/cloud/oauth/` — `error`, `provider`, `flow`, `loopback`, `refresh`, `actions/{start,poll,cancel,complete}`.
- [x] `OauthProvider` trait + `OauthProviderRegistry`.
- [x] `OauthFlow` state machine (`Pending` → `Completed` / `Failed` / `Cancelled`), `OauthFlowStore` backed by `DashMap` with per-flow `watch::Sender<bool>` cancellation channels.
- [x] Janitor (`run_janitor`) evicting pending flows after 10 minutes and terminal flows after a 30-second grace window.
- [x] One-shot loopback HTTP server with constant-time CSRF state validation, 5-minute timeout, cancellation-aware.
- [x] Library action `cloud.oauth.start` — validates inputs, binds loopback (exact-match registered ports or ephemeral), generates PKCE S256 + CSRF state, launches browser best-effort, spawns completion task.
- [x] Library query `cloud.oauth.poll` returning current flow status.
- [x] Library action `cloud.oauth.cancel` — signals loopback task and transitions flow to `Cancelled`.
- [x] Internal `complete_flow` driving loopback → `exchange_code` → `display_name` → store. Deliberately off the wire surface.
- [x] `CloudTokenRefreshTask` — rotates tokens 5 minutes before expiry across every library, tolerates unknown providers, persists rotated refresh tokens.
- [x] `CoreContext` carries shared `OauthFlowStore` and `OauthProviderRegistry`; both spawned at startup from `lib.rs`.
- [x] `OneDriveProvider` concrete implementation (`tenant=common`, scopes `Files.ReadWrite.All offline_access User.Read`, loopback ports `53682..=53686`, Graph `/me` display-name hydration, `select_account` prompt).
- [x] Frontend wiring in `AddStorageModal` → new `OneDriveConnectForm` with BYO tutorial (7 steps), `openExternal(auth_url)`, `refetchInterval: 1000` poll until `Completed` / `Failed` / `Cancelled`, backup "copy link" affordance for OS-default-browser failure.
- [x] `OneDriveChangeDetector` over Microsoft Graph `/me/drive/root/delta` with typed error mapping (`Invalidated` / `RateLimited` / `Auth`) and persisted `cloud_sync_state.change_token` advancement per page.
- [x] Sidebar context-menu "Disconnect" item wired to `volumes.remove_cloud`, unified `getVolumeIcon` between sidebar and device panel, `GroupType::Cloud` hidden from AddGroup / SpaceCustomization dropdowns with TODO comment pointing at the missing renderer.
- [x] `CloudCopyStrategy` covering same-backend server-side copy with streaming fallback, local↔cloud streaming, cross-backend streaming, and a typed error for the no-cloud-endpoint routing bug.
- [x] Integration test `core/tests/onedrive_end_to_end_test.rs` — three `#[tokio::test]` cases against wiremock: connect-and-disconnect journey, cancel path, token-exchange failure surfacing provider error.
- [x] Unit tests across every component: PKCE S256 vector, flow TTL janitor, loopback happy/state-mismatch/user-denied/timeout/cancel, input validation, registry insert/overwrite, refresh heuristics, detector 410/429/401 error classification, path normalization, provider URL construction.

## Acceptance Criteria

- [x] `cloud.oauth.start`, `cloud.oauth.poll`, `cloud.oauth.cancel` reach the RPC registry.
- [x] `CoreContext` carries a shared `OauthFlowStore` and `OauthProviderRegistry`.
- [x] CSRF state compared in constant time; PKCE S256 matches RFC 7636 Appendix B.
- [x] Loopback returns minimal success HTML on capture, 400 on malformed callbacks.
- [x] Pending flows older than 10 minutes evicted; terminal flows survive a 30-second grace.
- [x] `OneDriveProvider` handles token exchange, refresh, and display-name hydration end to end against wiremock.
- [x] `OneDriveChangeDetector` advances `change_token` per page and classifies 410/429/401 into `Invalidated`/`RateLimited`/`Auth`.
- [x] UI connect flow works end to end in the Tauri dev build.
- [x] `cargo build -p sd-core`, `cargo clippy -p sd-core --lib --no-deps`, `cargo test -p sd-core --lib`, and `cargo test -p sd-core --test onedrive_end_to_end_test` all pass.

## Carried-Over Tech Debt

Items delivered elsewhere in the MVP but not yet closed. Each has a `TODO(cloud-mvp)` in the code pointing at this proposal.

1. **Hot-swap `CloudBackend` on token refresh.** `CloudTokenRefreshTask` rewrites the credential row in SQLite, but the active `CloudBackend` instance in `VolumeManager` keeps the stale access token until the volume is reloaded. A follow-up adds `VolumeManager::reload_credentials(volume_id)` to rebuild and atomically swap the backend. Tracked at `core/src/ops/cloud/change_detection/onedrive.rs` and `core/src/ops/cloud/oauth/refresh.rs`.
2. **Delta stream not consumed to skip re-hashing.** `OneDriveChangeDetector` advances the token, but the indexer still treats every cloud entry as new on each full pass. Wiring the detector output into `phases::processing` is the next change-detection PR.
3. **Backblaze B2, Wasabi, DigitalOcean Spaces rehydration gap.** `VolumeManager::restore_cloud_volumes` still has a catch-all `warn!` for those three variants. Pre-existing before the MVP; carried over because it blocks no OneDrive user but needs closure for a complete cloud story.

## Next Steps (outside CLOUD-004)

- **`GoogleDriveProvider`** — reuse the `OauthProvider` trait; scopes `drive.file` to avoid Google OAuth verification; ephemeral loopback port; paste-tokens flow in the UI swaps for a Connect button mirroring OneDrive.
- **`DropboxProvider`** — reuse the trait; Dropbox requires exact-match redirects like Microsoft, so register the same 5 loopback ports.
- **Reconnect / edit-credentials UX** for the refresh-task give-up case (currently: delete + re-add).
- **OneDrive Business / SharePoint tenants** — requires tenant-specific issuer; an extension of `OneDriveProvider` with an optional tenant parameter.

## Out of Scope

- Spacedrive-owned public OAuth apps (BYO is the MVP choice; phase 2 decision).
- Device-code flow for CLI-over-SSH.
- LIST-diff change detection for S3 / GCS / Azure Blob.
- Cross-backend server-side copy.
