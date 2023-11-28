//! Android file system watcher implementation.

use crate::{
	invalidate_query, library::Library, location::manager::LocationManagerError, prisma::location,
	util::error::FileIOError, Node,
};

use std::{
	collections::{BTreeMap, HashMap},
	path::{Path, PathBuf},
	sync::Arc,
};

use async_trait::async_trait;
use notify::{
	event::{CreateKind, DataChange, ModifyKind, RenameMode},
	Event, EventKind,
};
use tokio::{fs, time::Instant};
use tracing::{error, trace};

use super::{
	utils::{create_dir, recalculate_directories_size, remove, rename, update_file},
	EventHandler, HUNDRED_MILLIS, ONE_SECOND,
};

#[derive(Debug)]
pub(super) struct AndroidEventHandler<'lib> {
	location_id: location::id::Type,
	library: &'lib Arc<Library>,
	node: &'lib Arc<Node>,
	last_events_eviction_check: Instant,
	rename_from: HashMap<PathBuf, Instant>,
	recently_renamed_from: BTreeMap<PathBuf, Instant>,
	files_to_update: HashMap<PathBuf, Instant>,
	reincident_to_update_files: HashMap<PathBuf, Instant>,
	to_recalculate_size: HashMap<PathBuf, Instant>,
	path_and_instant_buffer: Vec<(PathBuf, Instant)>,
}

#[async_trait]
impl<'lib> EventHandler<'lib> for AndroidEventHandler<'lib> {
	fn new(
		location_id: location::id::Type,
		library: &'lib Arc<Library>,
		node: &'lib Arc<Node>,
	) -> Self {
		Self {
			location_id,
			library,
			node,
			last_events_eviction_check: Instant::now(),
			rename_from: HashMap::new(),
			recently_renamed_from: BTreeMap::new(),
			files_to_update: HashMap::new(),
			reincident_to_update_files: HashMap::new(),
			to_recalculate_size: HashMap::new(),
			path_and_instant_buffer: Vec::new(),
		}
	}
}

impl AndroidEventHandler<'_> {
	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError> {
		trace!("Received Android event: {:#?}", event);

		Ok(())
	}
}