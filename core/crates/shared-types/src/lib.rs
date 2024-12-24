pub mod cas_id;
pub mod core_event;
pub mod jobs;
pub mod kind_statistic;
pub mod notification;
pub mod sd_path;
pub mod thumbnail;
pub mod volume;

pub use jobs::metadata::*;

use serde::{Deserialize, Serialize};
use specta::Type;

pub type LibraryId = uuid::Uuid;

/// All of the feature flags provided by the core itself. The frontend has it's own set of feature flags!
///
/// If you want a variant of this to show up on the frontend it must be added to `backendFeatures` in `useFeatureFlag.tsx`
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BackendFeature {}

// impl BackendFeature {
// 	pub fn restore(&self, node: &Node) {
// 		match self {
// 			BackendFeature::CloudSync => {
// 				node.cloud_sync_flag.store(true, Ordering::Relaxed);
// 			}
// 		}
// 	}
// }
