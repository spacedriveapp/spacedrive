//! Get sync activity input

use serde::{Deserialize, Serialize};
use specta::Type;

/// Input for getting sync activity summary
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetSyncActivityInput {}
