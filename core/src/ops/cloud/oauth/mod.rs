//! # Cloud OAuth 2.0 infrastructure (BYO)
//!
//! Provider-agnostic OAuth 2.0 authorization-code flow with PKCE and loopback
//! redirect (RFC 8252) for desktop applications. Every method accepts the
//! user's own `client_id` and `client_secret` — Spacedrive never ships a
//! hardcoded public client, so the infrastructure must be fully "bring your
//! own app".
//!
//! ## Flow summary
//! 1. The UI calls [`actions::start::CloudOauthStartAction`] with `provider`,
//!    `client_id`, `client_secret`. The action spawns a one-shot loopback HTTP
//!    server, generates a CSRF state token and a PKCE S256 challenge, builds
//!    the authorization URL, and returns it.
//! 2. The UI opens the URL in the system browser. The browser redirects to
//!    `http://127.0.0.1:{port}` with `code` and `state` query params.
//! 3. The loopback server validates `state`, calls the provider's
//!    `exchange_code`, fetches a display name, and parks the resulting tokens
//!    in an in-memory flow state machine.
//! 4. The UI polls [`actions::poll::CloudOauthPollQuery`] and reads the final
//!    `TokenSet` + `display_name`, then hands them to `volumes.add_cloud` as
//!    before. No new domain object is introduced for the MVP.
//! 5. A background [`refresh::run_refresh_task`] iterates stored credentials
//!    and rotates access tokens before they expire.

pub mod actions;
pub mod error;
pub mod flow;
pub mod loopback;
pub mod provider;
pub mod providers;
pub mod refresh;

pub use error::OauthError;
pub use flow::{OauthFlow, OauthFlowStatus, OauthFlowStore};
pub use provider::{OauthProvider, OauthProviderRegistry, TokenSet};
pub use providers::OneDriveProvider;
