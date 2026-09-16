//! Thumbnail hot-tier ops.
//!
//! One action, called by whichever client is about to draw: it names the
//! paths in view, in the order they will be drawn, and gets back the cache
//! key for each. Anything not already fresh is queued for a bake, and the
//! completion arrives as a `thumbnail` resource event.

pub mod request;
pub mod thumbstrip;
