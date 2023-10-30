use crate::{
	invalidate_query,
	job::{JobBuilder, JobError, JobManagerError},
	library::Library,
	location::file_path_helper::filter_existing_file_path_params,
	object::{
		file_identifier::{self, file_identifier_job::FileIdentifierJobInit},
		media::{media_processor, MediaProcessorJobInit},
	},
	prisma::{file_path, indexer_rules_in_location, location, PrismaClient},
	util::{
		db::{maybe_missing, MissingFieldError},
		error::{FileIOError, NonUtf8PathError},
	},
	Node,
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
use sd_prisma::prisma_sync;
use sd_sync::*;
use sd_utils::uuid_to_bytes;
use serde::Deserialize;
use serde_json::json;
use specta::Type;
use tokio::{fs, io, time::Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

mod error;
pub mod file_path_helper;
pub mod indexer;
mod manager;
pub mod metadata;
pub mod non_indexed;

pub use error::LocationError;
use indexer::IndexerJobInit;
pub use manager::{LocationManagerError, Locations};
use metadata::SpacedriveLocationMetadataFile;

use file_path_helper::IsolatedFilePathData;

pub type LocationPubId = Uuid;

// Location includes!
location::include!(location_with_indexer_rules {
	indexer_rules: select { indexer_rule }
});

/// `LocationCreateArgs` is the argument received from the client using `rspc` to create a new location.
/// It has the actual path and a vector of indexer rules ids, to create many-to-many relationships
/// between the location and indexer rules.
#[derive(Type, Deserialize)]
pub struct LocationCreateArgs {
	pub path: PathBuf,
	pub dry_run: bool,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationCreateArgs {
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
			"{} new location for '{}'",
			if self.dry_run {
				"Dry run: Would create"
			} else {
				"Trying to create"
			},
			self.path.display()
		);

		let uuid = Uuid::new_v4();

		let location = create_location(
			library,
			uuid,
			&self.path,
			&self.indexer_rules_ids,
			self.dry_run,
		)
		.await?;

		if let Some(location) = location {
			// Write location metadata to a .spacedrive file
			if let Err(err) = SpacedriveLocationMetadataFile::create_and_save(
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
				delete_location(node, library, location.data.id).await?;
				Err(err)?;
			}

			info!("Created location: {:?}", &location.data);

			Ok(Some(location.data))
		} else {
			Ok(None)
		}
	}

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
			"{} a new Library <id='{}'> to an already existing location '{}'",
			if self.dry_run {
				"Dry run: Would add"
			} else {
				"Trying to add"
			},
			library.id,
			self.path.display()
		);

		let uuid = Uuid::new_v4();

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

			info!(
				"Added library (library_id = {}) to location: {:?}",
				library.id, &location.data
			);

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

		let (sync_params, db_params): (Vec<_>, Vec<_>) = [
			self.name
				.filter(|name| location.name.as_ref() != Some(name))
				.map(|v| {
					(
						(location::name::NAME, json!(v)),
						location::name::set(Some(v)),
					)
				}),
			self.generate_preview_media.map(|v| {
				(
					(location::generate_preview_media::NAME, json!(v)),
					location::generate_preview_media::set(Some(v)),
				)
			}),
			self.sync_preview_media.map(|v| {
				(
					(location::sync_preview_media::NAME, json!(v)),
					location::sync_preview_media::set(Some(v)),
				)
			}),
			self.hidden.map(|v| {
				(
					(location::hidden::NAME, json!(v)),
					location::hidden::set(Some(v)),
				)
			}),
			self.path.clone().map(|v| {
				(
					(location::path::NAME, json!(v)),
					location::path::set(Some(v)),
				)
			}),
		]
		.into_iter()
		.flatten()
		.unzip();

		if !sync_params.is_empty() {
			sync.write_ops(
				db,
				(
					sync_params
						.into_iter()
						.map(|p| {
							sync.shared_update(
								prisma_sync::location::SyncId {
									pub_id: location.pub_id.clone(),
								},
								p.0,
								p.1,
							)
						})
						.collect(),
					db.location()
						.update(location::id::equals(self.id), db_params),
				),
			)
			.await?;

			// TODO(N): This will probs fall apart with removable media.
			if location.instance_id == Some(library.config().instance_id) {
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

pub async fn scan_location(
	node: &Arc<Node>,
	library: &Arc<Library>,
	location: location_with_indexer_rules::Data,
) -> Result<(), JobManagerError> {
	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().instance_id) {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	JobBuilder::new(IndexerJobInit {
		location,
		sub_path: None,
	})
	.with_action("scan_location")
	.with_metadata(json!({"location": location_base_data.clone()}))
	.build()
	.queue_next(FileIdentifierJobInit {
		location: location_base_data.clone(),
		sub_path: None,
	})
	.queue_next(MediaProcessorJobInit {
		location: location_base_data,
		sub_path: None,
		regenerate_thumbnails: false,
	})
	.spawn(node, library)
	.await
	.map_err(Into::into)
}

pub async fn scan_location_sub_path(
	node: &Arc<Node>,
	library: &Arc<Library>,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path>,
) -> Result<(), JobManagerError> {
	let sub_path = sub_path.as_ref().to_path_buf();

	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().instance_id) {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	JobBuilder::new(IndexerJobInit {
		location,
		sub_path: Some(sub_path.clone()),
	})
	.with_action("scan_location_sub_path")
	.with_metadata(json!({
		"location": location_base_data.clone(),
		"sub_path": sub_path.clone(),
	}))
	.build()
	.queue_next(FileIdentifierJobInit {
		location: location_base_data.clone(),
		sub_path: Some(sub_path.clone()),
	})
	.queue_next(MediaProcessorJobInit {
		location: location_base_data,
		sub_path: Some(sub_path),
		regenerate_thumbnails: false,
	})
	.spawn(node, library)
	.await
	.map_err(Into::into)
}

pub async fn light_scan_location(
	node: Arc<Node>,
	library: Arc<Library>,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path>,
) -> Result<(), JobError> {
	let sub_path = sub_path.as_ref().to_path_buf();

	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id != Some(library.config().instance_id) {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	indexer::shallow(&location, &sub_path, &node, &library).await?;
	file_identifier::shallow(&location_base_data, &sub_path, &library).await?;
	media_processor::shallow(&location_base_data, &sub_path, &library, &node).await?;

	Ok(())
}

pub async fn relink_location(
	Library { db, id, sync, .. }: &Library,
	location_path: impl AsRef<Path>,
) -> Result<i32, LocationError> {
	let location_path = location_path.as_ref();
	let mut metadata = SpacedriveLocationMetadataFile::try_load(&location_path)
		.await?
		.ok_or_else(|| LocationError::MissingMetadataFile(location_path.into()))?;

	metadata.relink(*id, location_path).await?;

	let pub_id = metadata.location_pub_id(*id)?.as_ref().to_vec();
	let path = location_path
		.to_str()
		.map(str::to_string)
		.ok_or_else(|| NonUtf8PathError(location_path.into()))?;

	sync.write_op(
		db,
		sync.shared_update(
			prisma_sync::location::SyncId {
				pub_id: pub_id.clone(),
			},
			location::path::NAME,
			json!(path),
		),
		db.location().update(
			location::pub_id::equals(pub_id.clone()),
			vec![location::path::set(Some(path))],
		),
	)
	.await?;

	let location_id = db
		.location()
		.find_unique(location::pub_id::equals(pub_id))
		.select(location::select!({ id }))
		.exec()
		.await?
		.ok_or_else(|| {
			LocationError::MissingField(MissingFieldError::new("missing id of location"))
		})?;

	Ok(location_id.id)
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
				// This ensures we always receive a valid windows formated path
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
		.await? > 0
	{
		return Err(LocationError::LocationAlreadyExists(location_path.into()));
	}

	if check_nested_location(&location_path, db).await? {
		return Err(LocationError::NestedLocation(location_path.into()));
	}

	if dry_run {
		return Ok(None);
	}

	let date_created = Utc::now();

	let location = sync
		.write_ops(
			db,
			(
				sync.shared_create(
					prisma_sync::location::SyncId {
						pub_id: location_pub_id.as_bytes().to_vec(),
					},
					[
						(location::name::NAME, json!(&name)),
						(location::path::NAME, json!(&path)),
						(location::date_created::NAME, json!(date_created)),
						(
							location::instance::NAME,
							json!(prisma_sync::instance::SyncId {
								pub_id: uuid_to_bytes(sync.instance)
							}),
						),
					],
				),
				db.location()
					.create(
						location_pub_id.as_bytes().to_vec(),
						vec![
							location::name::set(Some(name.clone())),
							location::path::set(Some(path)),
							location::date_created::set(Some(date_created.into())),
							location::instance_id::set(Some(library.config().instance_id)),
							// location::instance::connect(instance::id::equals(
							// 	library.config.instance_id.as_bytes().to_vec(),
							// )),
						],
					)
					.include(location_with_indexer_rules::include()),
			),
		)
		.await?;

	debug!("New location created in db");

	if !indexer_rules_ids.is_empty() {
		link_location_and_indexer_rules(library, location.id, indexer_rules_ids).await?;
	}

	// Updating our location variable to include information about the indexer rules
	let location = find_location(library, location.id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location.id))?;

	invalidate_query!(library, "locations.list");

	Ok(Some(CreatedLocationResult {
		data: location,
		name,
	}))
}

pub async fn delete_location(
	node: &Node,
	library: &Arc<Library>,
	location_id: location::id::Type,
) -> Result<(), LocationError> {
	let start = Instant::now();
	node.locations.remove(location_id, library.clone()).await?;
	debug!(
		"Elapsed time to remove location from node: {:?}",
		start.elapsed()
	);

	let start = Instant::now();
	delete_directory(library, location_id, None).await?;
	debug!(
		"Elapsed time to delete location file paths: {:?}",
		start.elapsed()
	);

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
	if location.instance_id == Some(library.config().instance_id) {
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
	debug!(
		"Elapsed time to remove location metadata: {:?}",
		start.elapsed()
	);

	let start = Instant::now();

	library
		.db
		.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location_id,
		)])
		.exec()
		.await?;
	debug!(
		"Elapsed time to delete indexer rules in location: {:?}",
		start.elapsed()
	);

	let start = Instant::now();

	library
		.db
		.location()
		.delete(location::id::equals(location_id))
		.exec()
		.await?;

	debug!(
		"Elapsed time to delete location from db: {:?}",
		start.elapsed()
	);

	invalidate_query!(library, "locations.list");

	info!("Location {location_id} deleted");

	Ok(())
}

/// Will delete a directory recursively with Objects if left as orphans
/// this function is used to delete a location and when ingesting directory deletion events
pub async fn delete_directory(
	library: &Library,
	location_id: location::id::Type,
	parent_iso_file_path: Option<&IsolatedFilePathData<'_>>,
) -> Result<(), QueryError> {
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

	db.file_path().delete_many(children_params).exec().await?;

	library.orphan_remover.invoke().await;

	invalidate_query!(library, "search.paths");
	invalidate_query!(library, "search.objects");

	Ok(())
}

impl From<location_with_indexer_rules::Data> for location::Data {
	fn from(data: location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id,
			path: data.path,
			instance_id: data.instance_id,
			name: data.name,
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			is_archived: data.is_archived,
			size_in_bytes: data.size_in_bytes,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			file_paths: None,
			indexer_rules: None,
			instance: None,
		}
	}
}

impl From<&location_with_indexer_rules::Data> for location::Data {
	fn from(data: &location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id.clone(),
			path: data.path.clone(),
			instance_id: data.instance_id,
			name: data.name.clone(),
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			size_in_bytes: data.size_in_bytes.clone(),
			is_archived: data.is_archived,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			file_paths: None,
			indexer_rules: None,
			instance: None,
		}
	}
}

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
				"Missing location path on location <id='{}'> at check nested location",
				v.id
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

pub async fn update_location_size(
	location_id: location::id::Type,
	library: &Library,
) -> Result<(), QueryError> {
	let Library { db, .. } = library;

	let total_size = db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location_id)),
			file_path::materialized_path::equals(Some("/".to_string())),
		])
		.select(file_path::select!({ size_in_bytes_bytes }))
		.exec()
		.await?
		.into_iter()
		.filter_map(|file_path| {
			file_path.size_in_bytes_bytes.map(|size_in_bytes_bytes| {
				u64::from_be_bytes([
					size_in_bytes_bytes[0],
					size_in_bytes_bytes[1],
					size_in_bytes_bytes[2],
					size_in_bytes_bytes[3],
					size_in_bytes_bytes[4],
					size_in_bytes_bytes[5],
					size_in_bytes_bytes[6],
					size_in_bytes_bytes[7],
				])
			})
		})
		.sum::<u64>();

	db.location()
		.update(
			location::id::equals(location_id),
			vec![location::size_in_bytes::set(Some(
				total_size.to_be_bytes().to_vec(),
			))],
		)
		.exec()
		.await?;

	invalidate_query!(library, "locations.list");
	invalidate_query!(library, "locations.get");

	Ok(())
}

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
