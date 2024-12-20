use crate::thumb_key::ThumbKey;
use crate::{jobs::progress::JobProgressEvent, kind_statistic::KindStatistic, LibraryId};
use sd_prisma::prisma::file_path;
use serde::Serialize;
use specta::Type;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail {
		thumb_key: ThumbKey,
	},
	NewIdentifiedObjects {
		file_path_ids: Vec<file_path::id::Type>,
	},
	UpdatedKindStatistic(KindStatistic, LibraryId),
	JobProgress(JobProgressEvent),
	// InvalidateOperation(InvalidateOperationEvent),
}
