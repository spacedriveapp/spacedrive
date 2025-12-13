//! Location list output types
//!
//! Note: The canonical resource type for locations is `crate::domain::Location`.
//! This module provides query-specific output wrappers.

use crate::domain::Location;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output for location list queries
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListOutput {
	pub locations: Vec<Location>,
}
