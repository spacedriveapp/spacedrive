use serde::{Deserialize, Serialize};
use specta::Type;

use crate::service::network::protocol::pairing::VouchingSession;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VouchingSessionOutput {
	pub session: Option<VouchingSession>,
}
