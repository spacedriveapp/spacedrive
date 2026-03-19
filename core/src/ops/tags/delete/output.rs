//! Output for delete tag action

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeleteTagOutput {
	pub deleted: bool,
}
