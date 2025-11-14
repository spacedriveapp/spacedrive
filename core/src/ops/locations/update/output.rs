//! Location update output

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationUpdateOutput {
	/// UUID of the updated location
	pub id: Uuid,
}
