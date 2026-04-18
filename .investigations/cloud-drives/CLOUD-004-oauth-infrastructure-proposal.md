---
id: CLOUD-004
title: Cloud OAuth Infrastructure (BYO)
status: In Progress
assignee: jamiepine
parent: CLOUD-003
priority: High
tags: [cloud, oauth, security]
whitepaper: Section 5.2
related_tasks: [CLOUD-003]
last_updated: 2026-04-18
---

## Description

Build the provider-agnostic OAuth 2.0 infrastructure that lets users connect cloud accounts (OneDrive, Google Drive, Dropbox) through a browser sign-in flow rather than pasting access/refresh tokens by hand.

Architecture is strictly Bring Your Own: the user supplies `client_id` and `client_secret` from an app they registered themselves in the provider's developer console. Spacedrive never ships hardcoded OAuth clients. `CloudStorageConfig::OneDrive` stays unchanged — OAuth is a companion subsystem that hands finished tokens back to `volumes.add_cloud`.

See `.investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#set-3` for the full plan. This task delivers the infrastructure; Set 4 (CLOUD-005, not yet filed) registers the first concrete provider (OneDrive).

## Implementation Steps

- [x] Add deps: `oauth2 = "5"`, `webbrowser = "1"`, `dashmap = "6"`, `subtle = "2"`, `base64 = "0.22"`.
- [x] Module tree under `core/src/ops/cloud/oauth/`: `error`, `provider`, `flow`, `loopback`, `refresh`, `actions/{start,poll,cancel,complete}`.
- [x] `OauthProvider` trait + `OauthProviderRegistry` (starts empty until Set 4 registers OneDrive).
- [x] `OauthFlow` state machine with `Pending/Completed/Failed/Cancelled` statuses, `OauthFlowStore` backed by `DashMap` + per-flow `watch::Sender<bool>` for cancellation.
- [x] Janitor (`run_janitor`) evicting pending flows after 10 minutes and terminal flows after a 30-second grace period; spawned from `lib.rs` at startup.
- [x] One-shot loopback HTTP server with CSRF state validation via `subtle::ConstantTimeEq`, UTF-8 query parsing, minimal success HTML response, 5-minute timeout, cancellation-aware.
- [x] Library action `cloud.oauth.start` — validates BYO inputs, binds loopback listener (exact-match registered ports or OS-assigned ephemeral), generates PKCE S256 + CSRF state, spawns the completion task, opens the system browser best-effort.
- [x] Library query `cloud.oauth.poll` returning the flow's current status.
- [x] Library action `cloud.oauth.cancel` signalling the loopback task and transitioning the flow to `Cancelled`.
- [x] Internal `complete_flow` that drives the loopback → `exchange_code` → `display_name` → store pipeline. Deliberately unregistered from the wire surface.
- [x] `CloudTokenRefreshTask`: rotates OAuth tokens 5 minutes before expiry across every library and credential, tolerates unknown providers, persists rotated refresh tokens when the provider rotates. Spawned from `lib.rs` at startup.
- [x] Attach `oauth_flows: OauthFlowStore` and `oauth_providers: OauthProviderRegistry` to `CoreContext`.
- [x] Unit tests: PKCE S256 vector, flow TTL janitor, loopback happy path + state mismatch + user-denied + timeout + cancel, input validation, registry insert/overwrite, refresh heuristics.

## Acceptance Criteria

- [x] `cloud.oauth.start`, `cloud.oauth.poll`, `cloud.oauth.cancel` reach the RPC registry.
- [x] `CoreContext` carries a shared `OauthFlowStore` and `OauthProviderRegistry`.
- [x] CSRF state is compared in constant time; PKCE S256 digest matches RFC 7636 Appendix B vector.
- [x] Loopback server returns a minimal success HTML on capture and a 400 on malformed callbacks.
- [x] Pending flows older than 10 minutes are evicted by the janitor; terminal flows survive a 30-second grace window.
- [x] Refresh task tolerates unknown providers (no errors until Set 4 registers OneDrive) and persists rotated refresh tokens.
- [x] `cargo build -p sd-core`, `cargo clippy -p sd-core --lib`, and `cargo test -p sd-core --lib` pass without new warnings in cloud oauth code.

## Implementation Files

**Subsystem:**

- `core/src/ops/cloud/mod.rs`
- `core/src/ops/cloud/oauth/mod.rs`
- `core/src/ops/cloud/oauth/error.rs`
- `core/src/ops/cloud/oauth/provider.rs`
- `core/src/ops/cloud/oauth/flow.rs`
- `core/src/ops/cloud/oauth/loopback.rs`
- `core/src/ops/cloud/oauth/refresh.rs`
- `core/src/ops/cloud/oauth/actions/mod.rs`
- `core/src/ops/cloud/oauth/actions/start.rs`
- `core/src/ops/cloud/oauth/actions/poll.rs`
- `core/src/ops/cloud/oauth/actions/cancel.rs`
- `core/src/ops/cloud/oauth/actions/complete.rs`

**Wiring:**

- `core/Cargo.toml` — new OAuth deps.
- `core/src/context.rs` — `oauth_flows` and `oauth_providers` fields on `CoreContext`.
- `core/src/lib.rs` — `tokio::spawn` for `run_janitor` and `run_refresh_task` at startup.
- `core/src/ops/mod.rs` — `pub mod cloud;`.

## Next Steps

1. **Set 4 — Register OneDrive provider** (new task, CLOUD-005). Implement `OneDriveProvider` with `tenant=common`, scopes `Files.ReadWrite.All offline_access User.Read`, loopback ports `53682..=53686` (Microsoft exact-match), Graph `/me` display name.
2. **Set 5 — Frontend UI** wiring the three actions into `AddStorageModal` + BYO tutorial.
3. **TODO(cloud-mvp): hot-swap CloudBackend on token refresh — see `.investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#set-3`.** The refresh task rewrites the credential row, but the active `CloudBackend` in `VolumeManager` still holds the stale access token until the volume is reloaded. A follow-up adds `VolumeManager::reload_credentials(volume_id)` to rebuild and atomically swap the backend.

## Out of Scope

- Concrete providers (OneDrive in Set 4, Google Drive / Dropbox later).
- UI changes (Set 5).
- Delta change detection (Set 6).
- Device-code flow for CLI-over-SSH — future enhancement.
