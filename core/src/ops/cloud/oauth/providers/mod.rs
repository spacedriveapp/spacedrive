//! OAuth provider implementations.
//!
//! Each submodule wires a concrete issuer (Microsoft, Google, Dropbox) to the
//! provider-agnostic [`super::provider::OauthProvider`] trait. Providers are
//! registered with [`super::provider::OauthProviderRegistry`] at core startup;
//! the trait object is the only thing the flow state machine sees.

pub mod onedrive;

pub use onedrive::OneDriveProvider;
