//! Navigation focus: where each client window is currently looking.
//!
//! One window publishes the directory it is browsing, and the search it is
//! running when it runs one, and other windows follow it, which is how the
//! Photos app tracks a file explorer window. Focus is
//! presence, not state: it lives in memory for the daemon's lifetime, is never
//! persisted, and is never synced between devices. A daemon restart comes back
//! with no focus at all and every window keeps showing what it was showing.
//!
//! Focus is keyed by group so two explorer windows on different directories do
//! not fight. A follower joins a group and receives that group's focus.

pub mod focus;
pub mod get;
pub mod set;

pub use focus::{FocusRegistry, NavigationFocus};
