use crate::NonCriticalJobError;

use sd_prisma::prisma::location;
use sd_task_system::TaskId;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use uuid::Uuid;

use super::IdentifiedFile;

pub struct ObjectProcessorTask {
	id: TaskId,
	location: Arc<location::Data>,
	location_path: Arc<PathBuf>,
	identified_files: HashMap<Uuid, IdentifiedFile>,
	read_metadata_time: Duration,
	errors: Vec<NonCriticalJobError>,
	is_shallow: bool,
}
