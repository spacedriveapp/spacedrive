//! SpaceUI-shaped components over gpui, named after their
//! `@spacedrive/primitives` counterparts. The web rendering is the spec.
//!
//! This module depends only on gpui crates and `theme` — never on app, data,
//! or view code — so it extracts to the spaceui repo later.

mod button;

pub use button::{Button, ButtonVariant};
