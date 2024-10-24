use crate::{context::NodeContext, invalidate_query, library::Library, Node};

use sd_core_file_path_helper::{
	filter_existing_file_path_params, IsolatedFilePathData, IsolatedFilePathDataParts,
};
use sd_core_heavy_lifting::{
	file_identifier::{self, FileIdentifier},
	indexer::{self, job::Indexer},
	job_system::report::ReportInputMetadata,
	media_processor::{self, job::MediaProcessor},
	JobEnqueuer, JobId,
};
use sd_core_prisma_helpers::{location_with_indexer_rules, CasId};

use sd_prisma::{
	prisma::{device, file_path, indexer_rules_in_location, instance, location, PrismaClient},
	prisma_sync,
};
use sd_sync::*;
use sd_utils::{
	db::{maybe_missing, size_in_bytes_from_db, size_in_bytes_to_db},
	error::{FileIOError, NonUtf8PathError},
	uuid_to_bytes,
};

use std::{
	collections::HashSet,
	path::{Component, Path, PathBuf},
	sync::Arc,
};

use chrono::Utc;
use futures::future::TryFutureExt;
use normpath::PathExt;
use prisma_client_rust::{operator::and, or, QueryError};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::{fs, io, time::Instant};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

mod error;
mod manager;
pub mod metadata;
pub mod non_indexed;

pub use error::LocationError;
pub use manager::{LocationManagerError, Locations};
use metadata::SpacedriveLocationMetadataFile;

pub type LocationPubId = Uuid;

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum ScanState {
	Pending = 0,
	Indexed = 1,
	FilesIdentified = 2,
	Completed = 3,
}

impl TryFrom<i32> for ScanState {
	type Error = LocationError;

	fn try_from(value: i32) -> Result<Self, Self::Error> {
		Ok(match value {
			0 => Self::Pending,
			1 => Self::Indexed,
			2 => Self::FilesIdentified,
			3 => Self::Completed,
			_ => return Err(LocationError::InvalidScanStateValue(value)),
		})
	}
}

/// `LocationCreateArgs` is the argument received from the client using `rspc` to create a new location.
/// It has the actual path and a vector of indexer rules ids, to create many-to-many relationships
/// between the location and indexer rules.
#[derive(Debug, Type, Deserialize)]
pub struct LocationCreateArgs {
	pub path: PathBuf,
	pub dry_run: bool,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationCreateArgs {
	#[instrument(skip(node, library), err)]
	pub async fn create(
		self,
		node: &Node,
		library: &Arc<Library>,
	) -> Result<Option<location_with_indexer_rules::Data>, LocationError> {
		let Some(path_str) = self.path.to_str().map(str::to_string) else {
			return Err(LocationError::NonUtf8Path(NonUtf8PathError(
				self.path.into_boxed_path(),
			)));
		};

		let path_metadata = match fs::metadata(&self.path).await {
			Ok(metadata) => metadata,
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				return Err(LocationError::PathNotFound(self.path.into_boxed_path()))
			}
			Err(e) => {
				return Err(LocationError::LocationPathFilesystemMetadataAccess(
					FileIOError::from((self.path, e)),
				));
			}
		};

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path.into_boxed_path()));
		}

		if let Some(mut metadata) = SpacedriveLocationMetadataFile::try_load(&self.path).await? {
			metadata
				.clean_stale_libraries(
					&node
						.libraries
						.get_all()
						.await
						.into_iter()
						.map(|library| library.id)
						.collect(),
				)
				.await?;

			if !metadata.is_empty() {
				if let Some(old_path) = metadata.location_path(library.id) {
					if old_path == self.path {
						if library
							.db
							.location()
							.count(vec![location::path::equals(Some(path_str))])
							.exec()
							.await? > 0
						{
							// Location already exists in this library
							return Err(LocationError::LocationAlreadyExists(
								self.path.into_boxed_path(),
							));
						}
					} else {
						return Err(LocationError::NeedRelink {
							old_path: old_path.into(),
							new_path: self.path.into_boxed_path(),
						});
					}
				} else {
					return Err(LocationError::AddLibraryToMetadata(
						self.path.into_boxed_path(),
					));
				};
			}
		}

		debug!(
			"{} new location",
			if self.dry_run {
				"Dry run: Would create"
			} else {
				"Trying to create"
			}
		);

		let uuid = Uuid::now_v7();

		let location = create_location(
			library,
			uuid,
			&self.path,
			&self.indexer_rules_ids,
			self.dry_run,
		)
		.await?;

		if let Some(location) = location {
			info!(location_name = ?location.name, "Created location;");

			// Write location metadata to a .spacedrive file
			if let Err(e) = SpacedriveLocationMetadataFile::create_and_save(
				library.id,
				uuid,
				&self.path,
				location.name,
			)
			.err_into::<LocationError>()
			.and_then(|()| async move {
				node.locations
					.add(location.data.id, library.clone())
					.await
					.map_err(Into::into)
			})
			.await
			{
				// DISABLED TO FAIL SILENTLY - HOTFIX FOR LACK OF WRITE PERMISSION PREVENTING LOCATION CREATION
				error!(?e, "Failed to write .spacedrive file;");
				// delete_location(node, library, location.data.id).await?;
				// Err(e)?;
			}

			Ok(Some(location.data))
		} else {
			Ok(None)
		}
	}

	#[instrument(skip(node, library), fields(library_id = %library.id), err)]
	pub async fn add_library(
		self,
		node: &Node,
		library: &Arc<Library>,
	) -> Result<Option<location_with_indexer_rules::Data>, LocationError> {
		let Some(mut metadata) = SpacedriveLocationMetadataFile::try_load(&self.path).await? else {
			return Err(LocationError::MetadataNotFound(self.path.into_boxed_path()));
		};

		metadata
			.clean_stale_libraries(
				&node
					.libraries
					.get_all()
					.await
					.into_iter()
					.map(|library| library.id)
					.collect(),
			)
			.await?;

		if metadata.has_library(library.id) {
			return Err(LocationError::NeedRelink {
				old_path: metadata
					.location_path(library.id)
					.expect("We checked that we have this library_id")
					.into(),
				new_path: self.path.into_boxed_path(),
			});
		}

		debug!(
			"{} a new Library to an already existing location",
			if self.dry_run {
				"Dry run: Would add"
			} else {
				"Trying to add"
			},
		);

		let uuid = Uuid::now_v7();

		let location = create_location(
			library,
			uuid,
			&self.path,
			&self.indexer_rules_ids,
			self.dry_run,
		)
		.await?;

		if let Some(location) = location {
			metadata
				.add_library(library.id, uuid, &self.path, location.name)
				.await?;

			node.locations
				.add(location.data.id, library.clone())
				.await?;

			info!(location_id = %location.data.id, "Added library to location;");

			Ok(Some(location.data))
		} else {
			Ok(None)
		}
	}
}

/// `LocationUpdateArgs` is the argument received from the client using `rspc` to update a location.
/// It contains the id of the location to be updated, possible a name to change the current location's name
/// and a vector of indexer rules ids to add or remove from the location.
///
/// It is important to note that only the indexer rule ids in this vector will be used from now on.
/// Old rules that aren't in this vector will be purged.
#[derive(Type, Deserialize)]
pub struct LocationUpdateArgs {
	id: location::id::Type,
	name: Option<String>,
	generate_preview_media: Option<bool>,
	sync_preview_media: Option<bool>,
	hidden: Option<bool>,
	indexer_rules_ids: Vec<i32>,
	path: Option<String>,
}

impl LocationUpdateArgs {
	pub async fn update(self, node: &Node, library: &Arc<Library>) -> Result<(), LocationError> {
		let Library { sync, db, .. } = &**library;

		let location = find_location(library, self.id)
			.include(location_with_indexer_rules::include())
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(self.id))?;

		let name = self.name.clone();

		let (sync_params, db_params) = [
			option_sync_db_entry!(
				self.name
					.filter(|name| location.name.as_ref() != Some(name)),
				location::name
			),
			option_sync_db_entry!(
				self.generate_preview_media,
				location::generate_preview_media
			),
			option_sync_db_entry!(self.sync_preview_media, location::sync_preview_media),
			option_sync_db_entry!(self.hidden, location::hidden),
			option_sync_db_entry!(self.path.clone(), location::path),
		]
		.into_iter()
		.flatten()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		if !sync_params.is_empty() {
			sync.write_op(
				db,
				sync.shared_update(
					prisma_sync::location::SyncId {
						pub_id: location.pub_id.clone(),
					},
					sync_params,
				),
				db.location()
					.update(location::id::equals(self.id), db_params)
					.select(location::select!({ id })),
			)
			.await?;

			// TODO(N): This will probs fall apart with removable media.
			if location.instance_id == Some(library.config().await.instance_id) {
				if let Some(path) = &location.path {
					if let Some(mut metadata) =
						SpacedriveLocationMetadataFile::try_load(path).await?
					{
						metadata
							.update(library.id, maybe_missing(name, "location.name")?)
							.await?;
					}
				}
			}

			if self.path.is_some() {
				node.locations.remove(self.id, library.clone()).await?;
				node.locations.add(self.id, library.clone()).await?;
			}
		}

		let current_rules_ids = location
			.indexer_rules
			.iter()
			.map(|r| r.indexer_rule.id)
			.collect::<HashSet<_>>();

		let new_rules_ids = self.indexer_rules_ids.into_iter().collect::<HashSet<_>>();

		if current_rules_ids != new_rules_ids {
			let rule_ids_to_add = new_rules_ids
				.difference(&current_rules_ids)
				.copied()
				.collect::<Vec<_>>();
			let rule_ids_to_remove = current_rules_ids
				.difference(&new_rules_ids)
				.copied()
				.collect::<Vec<_>>();

			if !rule_ids_to_remove.is_empty() {
				library
					.db
					.indexer_rules_in_location()
					.delete_many(vec![
						indexer_rules_in_location::location_id::equals(self.id),
						indexer_rules_in_location::indexer_rule_id::in_vec(rule_ids_to_remove),
					])
					.exec()
					.await?;
			}

			if !rule_ids_to_add.is_empty() {
				link_location_and_indexer_rules(library, self.id, &rule_ids_to_add).await?;
			}
		}

		Ok(())
	}
}

pub fn find_location(
	library: &Library,
	location_id: location::id::Type,
) -> location::FindUniqueQuery {
	library
		.db
		.location()
		.find_unique(location::id::equals(location_id))
}

async fn link_location_and_indexer_rules(
	library: &Library,
	location_id: location::id::Type,
	rules_ids: &[i32],
) -> Result<(), LocationError> {
	library
		.db
		.indexer_rules_in_location()
		.create_many(
			rules_ids
				.iter()
				.map(|id| indexer_rules_in_location::create_unchecked(location_id, *id, vec![]))
				.collect(),
		)
		.exec()
		.await?;

	Ok(())
}

#[instrument(
	skip(node, library, location),
	fields(library_id = %library.id, location_id = %location.id),
	err,
)]
pub async fn scan_location(
	node: &Arc<Node>,
	library: &Arc<Library>,
	location: location_with_indexer_rules::Data,
	location_scan_state: ScanState,
) -> Result<Option<JobId>, sd_core_heavy_lifting::Error> {
	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().await.instance_id) {
		warn!("Tried to scan a location on a different instance");
		return Ok(None);
	}

	let location_id = location.id;
	let ctx = NodeContext {
		node: Arc::clone(node),
		library: Arc::clone(library),
	};

	let location_base_data = location::Data::from(&location);

	debug!("Scanning location");

	let job_id = match location_scan_state {
		ScanState::Pending | ScanState::Completed => {
			node.job_system
				.dispatch(
					JobEnqueuer::new(Indexer::new(location, None)?)
						.with_action("scan_location")
						.with_metadata(ReportInputMetadata::Location(location_base_data.clone()))
						.enqueue_next(FileIdentifier::new(location_base_data.clone(), None)?)
						.enqueue_next(MediaProcessor::new(location_base_data, None, false)?),
					location_id,
					ctx.clone(),
				)
				.await?
		}

		ScanState::Indexed => {
			node.job_system
				.dispatch(
					JobEnqueuer::new(FileIdentifier::new(location_base_data.clone(), None)?)
						.with_action("scan_location_already_indexed")
						.with_metadata(ReportInputMetadata::Location(location_base_data.clone()))
						.enqueue_next(MediaProcessor::new(location_base_data, None, false)?),
					location_id,
					ctx.clone(),
				)
				.await?
		}

		ScanState::FilesIdentified => {
			node.job_system
				.dispatch(
					JobEnqueuer::new(MediaProcessor::new(
						location_base_data.clone(),
						None,
						false,
					)?)
					.with_action("scan_location_files_already_identified")
					.with_metadata(ReportInputMetadata::Location(location_base_data)),
					location_id,
					ctx.clone(),
				)
				.await?
		}
	};

	Ok(Some(job_id))
}

#[instrument(
	skip_all,
	fields(
		library_id = %library.id,
		location_id = %location.id,
		sub_path = %sub_path.as_ref().display(),
	),
	err,
)]
pub async fn scan_location_sub_path(
	node: &Arc<Node>,
	library: &Arc<Library>,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path> + Send,
) -> Result<Option<JobId>, sd_core_heavy_lifting::Error> {
	let sub_path = sub_path.as_ref().to_path_buf();

	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().await.instance_id) {
		warn!("Tried to scan a location on a different instance");
		return Ok(None);
	}

	let location_id = location.id;
	let ctx = NodeContext {
		node: Arc::clone(node),
		library: Arc::clone(library),
	};

	let location_base_data = location::Data::from(&location);

	debug!("Scanning location on a sub path");

	node.job_system
		.dispatch(
			JobEnqueuer::new(Indexer::new(location, Some(sub_path.clone()))?)
				.with_action("scan_location")
				.with_metadata(ReportInputMetadata::Location(location_base_data.clone()))
				.with_metadata(ReportInputMetadata::SubPath(sub_path.clone()))
				.enqueue_next(FileIdentifier::new(
					location_base_data.clone(),
					Some(sub_path.clone()),
				)?)
				.enqueue_next(MediaProcessor::new(
					location_base_data,
					Some(sub_path),
					false,
				)?),
			location_id,
			ctx.clone(),
		)
		.await
		.map_err(Into::into)
		.map(Some)
}

#[instrument(
	skip_all,
	fields(
		library_id = %library.id,
		location_id = %location.id,
		sub_path = %sub_path.as_ref().display(),
	),
	err,
)]
pub async fn light_scan_location(
	node: Arc<Node>,
	library: Arc<Library>,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path>,
) -> Result<(), sd_core_heavy_lifting::Error> {
	let sub_path = sub_path.as_ref().to_path_buf();

	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().await.instance_id) {
		warn!("Tried to scan a location on a different instance");
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	let dispatcher = node.task_system.get_dispatcher();
	let ctx = NodeContext { node, library };

	for e in indexer::shallow(location, &sub_path, &dispatcher, &ctx).await? {
		error!(?e, "Shallow indexer errors;");
	}

	for e in
		file_identifier::shallow(location_base_data.clone(), &sub_path, &dispatcher, &ctx).await?
	{
		error!(?e, "Shallow file identifier errors;");
	}

	for e in media_processor::shallow(location_base_data, &sub_path, &dispatcher, &ctx).await? {
		error!(?e, "Shallow media processor errors;");
	}

	Ok(())
}

#[instrument(
	skip_all,
	fields(
		library_id = %id,
		location_path = %location_path.as_ref().display(),
	),
	err,
)]
pub async fn relink_location(
	Library { db, id, sync, .. }: &Library,
	location_path: impl AsRef<Path>,
) -> Result<location::id::Type, LocationError> {
	let location_path = location_path.as_ref();
	let mut metadata = SpacedriveLocationMetadataFile::try_load(&location_path)
		.await?
		.ok_or_else(|| LocationError::MissingMetadataFile(location_path.into()))?;

	metadata.relink(*id, location_path).await?;

	let pub_id = uuid_to_bytes(&metadata.location_pub_id(*id)?);
	let path = location_path
		.to_str()
		.map(str::to_string)
		.ok_or_else(|| NonUtf8PathError(location_path.into()))?;

	let (sync_param, db_param) = sync_db_entry!(path, location::path);

	let location_id = sync
		.write_op(
			db,
			sync.shared_update(
				prisma_sync::location::SyncId {
					pub_id: pub_id.clone(),
				},
				[sync_param],
			),
			db.location()
				.update(location::pub_id::equals(pub_id.clone()), vec![db_param])
				.select(location::select!({ id })),
		)
		.await?
		.id;

	Ok(location_id)
}

#[derive(Debug)]
pub struct CreatedLocationResult {
	pub name: String,
	pub data: location_with_indexer_rules::Data,
}

pub(crate) fn normalize_path(path: impl AsRef<Path>) -> io::Result<(String, String)> {
	let mut path = path.as_ref().to_path_buf();
	let (location_path, normalized_path) = path
		// Normalize path and also check if it exists
		.normalize()
		.and_then(|normalized_path| {
			if cfg!(windows) {
				// Use normalized path as main path on Windows
				// This ensures we always receive a valid windows formatted path
				// ex: /Users/JohnDoe/Downloads will become C:\Users\JohnDoe\Downloads
				// Internally `normalize` calls `GetFullPathNameW` on Windows
				// https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew
				path = normalized_path.as_path().to_path_buf();
			}

			Ok((
				// TODO: Maybe save the path bytes instead of the string representation to avoid depending on UTF-8
				path.to_str().map(str::to_string).ok_or(io::Error::new(
					io::ErrorKind::InvalidInput,
					"Found non-UTF-8 path",
				))?,
				normalized_path,
			))
		})?;

	// Not needed on Windows because the normalization already handles it
	if cfg!(not(windows)) {
		// Replace location_path with normalize_path, when the first one ends in `.` or `..`
		// This is required so localize_name doesn't panic
		if let Some(component) = path.components().next_back() {
			if matches!(component, Component::CurDir | Component::ParentDir) {
				path = normalized_path.as_path().to_path_buf();
			}
		}
	}

	// Use `to_string_lossy` because a partially corrupted but identifiable name is better than nothing
	let mut name = path.localize_name().to_string_lossy().to_string();

	// Windows doesn't have a root directory
	if cfg!(not(windows)) && name == "/" {
		name = "Root".to_string()
	}

	if name.replace(char::REPLACEMENT_CHARACTER, "") == "" {
		name = "Unknown".to_string()
	}

	Ok((location_path, name))
}

async fn create_location(
	library @ Library { db, sync, .. }: &Library,
	location_pub_id: Uuid,
	location_path: impl AsRef<Path>,
	indexer_rules_ids: &[i32],
	dry_run: bool,
) -> Result<Option<CreatedLocationResult>, LocationError> {
	let location_path = location_path.as_ref();
	let (path, name) = normalize_path(location_path)
		.map_err(|_| LocationError::DirectoryNotFound(location_path.into()))?;

	if db
		.location()
		.count(vec![location::path::equals(Some(path.clone()))])
		.exec()
		.await?
		> 0
	{
		return Err(LocationError::LocationAlreadyExists(location_path.into()));
	}

	if check_nested_location(&location_path, db).await? {
		return Err(LocationError::NestedLocation(location_path.into()));
	}

	if dry_run {
		return Ok(None);
	}

	let (sync_values, mut db_params) = [
		sync_db_entry!(&name, location::name),
		sync_db_entry!(path, location::path),
		sync_db_entry!(Utc::now(), location::date_created),
		(
			sync_entry!(
				prisma_sync::device::SyncId {
					pub_id: sync.device_pub_id.to_db()
				},
				location::device
			),
			location::device::connect(device::pub_id::equals(sync.device_pub_id.to_db())),
		),
	]
	.into_iter()
	.unzip::<_, _, Vec<_>, Vec<_>>();

	// temporary workaround until we remove instances from locations
	db_params.push(location::instance::connect(instance::id::equals(
		library.config().await.instance_id,
	)));

	let location_id = sync
		.write_op(
			db,
			sync.shared_create(
				prisma_sync::location::SyncId {
					pub_id: uuid_to_bytes(&location_pub_id),
				},
				sync_values,
			),
			db.location()
				.create(uuid_to_bytes(&location_pub_id), db_params)
				.select(location::select!({ id })),
		)
		.await?
		.id;

	debug!("New location created in db");

	if !indexer_rules_ids.is_empty() {
		link_location_and_indexer_rules(library, location_id, indexer_rules_ids).await?;
	}

	// Updating our location variable to include information about the indexer rules
	let location = find_location(library, location_id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location_id))?;

	invalidate_query!(library, "locations.list");

	Ok(Some(CreatedLocationResult {
		data: location,
		name,
	}))
}

#[instrument(skip(node, library), fields(library_id = %library.id), err)]
pub async fn delete_location(
	node: &Node,
	library: &Arc<Library>,
	location_id: location::id::Type,
) -> Result<(), LocationError> {
	let Library { db, sync, .. } = library.as_ref();

	let start = Instant::now();
	node.locations.remove(location_id, library.clone()).await?;
	debug!(elapsed_time = ?start.elapsed(), "Removed location from node;");

	let start = Instant::now();
	delete_directory(library, location_id, None).await?;
	debug!(elapsed_time = ?start.elapsed(), "Deleted location file paths;");

	let location = library
		.db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location_id))?;

	let start = Instant::now();
	// TODO: This should really be queued to the proper node so it will always run
	// TODO: Deal with whether a location is online or not
	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id == Some(library.config().await.instance_id) {
		if let Some(path) = &location.path {
			if let Ok(Some(mut metadata)) = SpacedriveLocationMetadataFile::try_load(path).await {
				metadata
					.clean_stale_libraries(
						&node
							.libraries
							.get_all()
							.await
							.into_iter()
							.map(|library| library.id)
							.collect(),
					)
					.await?;

				metadata.remove_library(library.id).await?;
			}
		}
	}
	debug!(elapsed_time = ?start.elapsed(), "Removed location metadata;");

	let start = Instant::now();

	library
		.db
		.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location_id,
		)])
		.exec()
		.await?;
	debug!(elapsed_time = ?start.elapsed(), "Deleted indexer rules in location;");

	let start = Instant::now();

	sync.write_op(
		db,
		sync.shared_delete(prisma_sync::location::SyncId {
			pub_id: location.pub_id,
		}),
		db.location().delete(location::id::equals(location_id)),
	)
	.await?;

	debug!(elapsed_time = ?start.elapsed(), "Deleted location from db;");

	invalidate_query!(library, "locations.list");

	info!("Location deleted");

	Ok(())
}

/// Will delete a directory recursively with Objects if left as orphans
/// this function is used to delete a location and when ingesting directory deletion events
#[instrument(skip_all, err)]
pub async fn delete_directory(
	library: &Library,
	location_id: location::id::Type,
	parent_iso_file_path: Option<&IsolatedFilePathData<'_>>,
) -> Result<(), sd_core_sync::Error> {
	let Library { db, .. } = library;

	let children_params = sd_utils::chain_optional_iter(
		[file_path::location_id::equals(Some(location_id))],
		[parent_iso_file_path.and_then(|parent| {
			parent
				.materialized_path_for_children()
				.map(|materialized_path| {
					or![
						and(filter_existing_file_path_params(parent)),
						file_path::materialized_path::starts_with(materialized_path),
					]
				})
		})],
	);

	let pub_ids = library
		.db
		.file_path()
		.find_many(children_params.clone())
		.select(file_path::select!({ pub_id }))
		.exec()
		.await?
		.into_iter()
		.map(|fp| fp.pub_id)
		.collect::<Vec<_>>();

	if pub_ids.is_empty() {
		debug!("No file paths to delete");
		return Ok(());
	}

	library
		.sync
		.write_ops(
			&library.db,
			(
				pub_ids
					.into_iter()
					.map(|pub_id| {
						library
							.sync
							.shared_delete(prisma_sync::file_path::SyncId { pub_id })
					})
					.collect(),
				db.file_path().delete_many(children_params),
			),
		)
		.await?;

	// library.orphan_remover.invoke().await;

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

#[instrument(skip_all, err)]
async fn check_nested_location(
	location_path: impl AsRef<Path>,
	db: &PrismaClient,
) -> Result<bool, QueryError> {
	let location_path = location_path.as_ref();

	let (parents_count, potential_children) = db
		._batch((
			db.location().count(vec![location::path::in_vec(
				location_path
					.ancestors()
					.skip(1) // skip the actual location_path, we only want the parents
					.map(|p| {
						p.to_str()
							.map(str::to_string)
							.expect("Found non-UTF-8 path")
					})
					.collect(),
			)]),
			db.location().find_many(vec![location::path::starts_with(
				location_path
					.to_str()
					.map(str::to_string)
					.expect("Found non-UTF-8 path"),
			)]),
		))
		.await?;

	let comps = location_path.components().collect::<Vec<_>>();
	let is_a_child_location = potential_children.into_iter().any(|v| {
		let Some(location_path) = v.path else {
			warn!(
				location_id = %v.id,
				"Missing location path on location at check nested location",
			);
			return false;
		};
		let comps2 = PathBuf::from(location_path);
		let comps2 = comps2.components().collect::<Vec<_>>();

		if comps.len() > comps2.len() {
			return false;
		}

		for (a, b) in comps.iter().zip(comps2.iter()) {
			if a != b {
				return false;
			}
		}

		true
	});

	Ok(parents_count > 0 || is_a_child_location)
}

#[instrument(skip_all, err)]
pub async fn update_location_size(
	location_id: location::id::Type,
	location_pub_id: location::pub_id::Type,
	library: &Library,
) -> Result<(), sd_core_sync::Error> {
	let Library { db, sync, .. } = library;

	let total_size = size_in_bytes_to_db(
		db.file_path()
			.find_many(vec![
				file_path::location_id::equals(Some(location_id)),
				file_path::materialized_path::equals(Some("/".to_string())),
			])
			.select(file_path::select!({ size_in_bytes_bytes }))
			.exec()
			.await?
			.into_iter()
			.filter_map(|file_path| {
				file_path
					.size_in_bytes_bytes
					.map(|size_in_bytes_bytes| size_in_bytes_from_db(&size_in_bytes_bytes))
			})
			.sum::<u64>(),
	);

	let (sync_param, db_param) = sync_db_entry!(total_size, location::size_in_bytes);

	sync.write_op(
		db,
		sync.shared_update(
			prisma_sync::location::SyncId {
				pub_id: location_pub_id,
			},
			[sync_param],
		),
		db.location()
			.update(location::id::equals(location_id), vec![db_param])
			.select(location::select!({ id })),
	)
	.await?;

	invalidate_query!(library, "locations.list");
	invalidate_query!(library, "locations.get");

	Ok(())
}

#[instrument(skip_all, err)]
pub async fn get_location_path_from_location_id(
	db: &PrismaClient,
	location_id: file_path::id::Type,
) -> Result<PathBuf, LocationError> {
	db.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await
		.map_err(Into::into)
		.and_then(|maybe_location| {
			maybe_location
				.ok_or(LocationError::IdNotFound(location_id))
				.and_then(|location| {
					location
						.path
						.map(PathBuf::from)
						.ok_or(LocationError::MissingPath(location_id))
				})
		})
}

#[instrument(skip_all, err)]
pub async fn create_file_path(
	crate::location::Library { db, sync, .. }: &crate::location::Library,
	IsolatedFilePathDataParts {
		materialized_path,
		is_dir,
		location_id,
		name,
		extension,
		..
	}: IsolatedFilePathDataParts<'_>,
	cas_id: Option<CasId<'_>>,
	metadata: sd_core_file_path_helper::FilePathMetadata,
) -> Result<file_path::Data, sd_core_file_path_helper::FilePathError> {
	use sd_utils::db::inode_to_db;

	use sd_prisma::prisma;

	let indexed_at = Utc::now();

	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.select(location::select!({ id pub_id }))
		.exec()
		.await?
		.ok_or(sd_core_file_path_helper::FilePathError::LocationNotFound(
			location_id,
		))?;

	let device_pub_id = sync.device_pub_id.to_db();

	let (sync_params, db_params) = [
		(
			sync_entry!(
				prisma_sync::location::SyncId {
					pub_id: location.pub_id
				},
				file_path::location
			),
			file_path::location::connect(prisma::location::id::equals(location.id)),
		),
		(
			sync_entry!(cas_id, file_path::cas_id),
			file_path::cas_id::set(cas_id.map(Into::into)),
		),
		sync_db_entry!(materialized_path, file_path::materialized_path),
		sync_db_entry!(name, file_path::name),
		sync_db_entry!(extension, file_path::extension),
		sync_db_entry!(
			size_in_bytes_to_db(metadata.size_in_bytes),
			file_path::size_in_bytes_bytes
		),
		sync_db_entry!(inode_to_db(metadata.inode), file_path::inode),
		sync_db_entry!(is_dir, file_path::is_dir),
		sync_db_entry!(metadata.created_at, file_path::date_created),
		sync_db_entry!(metadata.modified_at, file_path::date_modified),
		sync_db_entry!(indexed_at, file_path::date_indexed),
		sync_db_entry!(metadata.hidden, file_path::hidden),
		(
			sync_entry!(
				prisma_sync::device::SyncId {
					pub_id: device_pub_id.clone()
				},
				file_path::device
			),
			file_path::device::connect(prisma::device::pub_id::equals(device_pub_id)),
		),
	]
	.into_iter()
	.unzip::<_, _, Vec<_>, Vec<_>>();

	let pub_id = sd_utils::uuid_to_bytes(&Uuid::now_v7());

	sync.write_op(
		db,
		sync.shared_create(
			prisma_sync::file_path::SyncId {
				pub_id: pub_id.clone(),
			},
			sync_params,
		),
		db.file_path().create(pub_id, db_params),
	)
	.await
	.map_err(Into::into)
}
