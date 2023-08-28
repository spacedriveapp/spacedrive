use crate::{
	api::CoreEvent,
	invalidate_query,
	job::{JobBuilder, JobError, JobManagerError},
	library::Library,
	location::file_path_helper::filter_existing_file_path_params,
	object::{
		file_identifier::{self, file_identifier_job::FileIdentifierJobInit},
		preview::{
			can_generate_thumbnail_for_image, generate_image_thumbnail, get_thumb_key,
			get_thumbnail_path, shallow_thumbnailer, thumbnailer_job::ThumbnailerJobInit,
		},
	},
	prisma::{file_path, indexer_rules_in_location, location, PrismaClient},
	util::error::FileIOError,
	Node,
};

use std::{
	collections::HashSet,
	path::{Component, Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use sd_file_ext::extensions::ImageExtension;

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
use tokio::{fs, io};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

mod error;
pub mod file_path_helper;
pub mod indexer;
mod manager;
mod metadata;
pub mod non_indexed;

pub use error::LocationError;
use indexer::IndexerJobInit;
pub use manager::{LocationManagerError, Locations};
use metadata::SpacedriveLocationMetadataFile;

use file_path_helper::IsolatedFilePathData;

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
		let path_metadata = match fs::metadata(&self.path).await {
			Ok(metadata) => metadata,
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				return Err(LocationError::PathNotFound(self.path))
			}
			Err(e) => {
				return Err(LocationError::LocationPathFilesystemMetadataAccess(
					FileIOError::from((self.path, e)),
				));
			}
		};

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path));
		}

		if let Some(metadata) = SpacedriveLocationMetadataFile::try_load(&self.path).await? {
			return if let Some(old_path) = metadata.location_path(library.id) {
				if old_path == self.path {
					Err(LocationError::LocationAlreadyExists(self.path))
				} else {
					Err(LocationError::NeedRelink {
						old_path: old_path.to_path_buf(),
						new_path: self.path,
					})
				}
			} else {
				Err(LocationError::AddLibraryToMetadata(self.path))
			};
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
				Ok(node
					.locations
					.add(location.data.id, library.clone())
					.await?)
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
		let mut metadata = SpacedriveLocationMetadataFile::try_load(&self.path)
			.await?
			.ok_or_else(|| LocationError::MetadataNotFound(self.path.clone()))?;

		if metadata.has_library(library.id) {
			return Err(LocationError::NeedRelink {
				// SAFETY: This unwrap is ok as we checked that we have this library_id
				old_path: metadata
					.location_path(library.id)
					.expect("This unwrap is ok as we checked that we have this library_id")
					.to_path_buf(),
				new_path: self.path,
			});
		}

		debug!(
			"{} a new library (library_id = {}) to an already existing location '{}'",
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
	pub id: location::id::Type,
	pub name: Option<String>,
	pub generate_preview_media: Option<bool>,
	pub sync_preview_media: Option<bool>,
	pub hidden: Option<bool>,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationUpdateArgs {
	pub async fn update(self, library: &Arc<Library>) -> Result<(), LocationError> {
		let Library { sync, db, .. } = &**library;

		let location = find_location(library, self.id)
			.include(location_with_indexer_rules::include())
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(self.id))?;

		let (sync_params, db_params): (Vec<_>, Vec<_>) = [
			self.name
				.clone()
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
			if location.instance_id == Some(library.config.instance_id) {
				if let Some(path) = &location.path {
					if let Some(mut metadata) =
						SpacedriveLocationMetadataFile::try_load(path).await?
					{
						metadata
							.update(library.id, self.name.expect("TODO"))
							.await?;
					}
				}
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
	if location.instance_id != Some(library.config.instance_id) {
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
	.queue_next(ThumbnailerJobInit {
		location: location_base_data,
		sub_path: None,
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
	if location.instance_id != Some(library.config.instance_id) {
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
	.queue_next(ThumbnailerJobInit {
		location: location_base_data,
		sub_path: Some(sub_path),
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
	if location.instance_id != Some(library.config.instance_id) {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	indexer::shallow(&location, &sub_path, &node, &library).await?;
	file_identifier::shallow(&location_base_data, &sub_path, &library).await?;
	shallow_thumbnailer(&location_base_data, &sub_path, &library, &node).await?;

	Ok(())
}

pub async fn relink_location(
	library: &Arc<Library>,
	location_path: impl AsRef<Path>,
) -> Result<(), LocationError> {
	let Library { db, id, sync, .. } = &**library;

	let mut metadata = SpacedriveLocationMetadataFile::try_load(&location_path)
		.await?
		.ok_or_else(|| LocationError::MissingMetadataFile(location_path.as_ref().to_path_buf()))?;

	metadata.relink(*id, &location_path).await?;

	let pub_id = metadata.location_pub_id(library.id)?.as_ref().to_vec();
	let path = location_path
		.as_ref()
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();

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
			location::pub_id::equals(pub_id),
			vec![location::path::set(Some(path))],
		),
	)
	.await?;

	Ok(())
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
	library: &Arc<Library>,
	location_pub_id: Uuid,
	location_path: impl AsRef<Path>,
	indexer_rules_ids: &[i32],
	dry_run: bool,
) -> Result<Option<CreatedLocationResult>, LocationError> {
	let Library { db, sync, .. } = &**library;

	let (path, name) = normalize_path(&location_path)
		.map_err(|_| LocationError::DirectoryNotFound(location_path.as_ref().to_path_buf()))?;

	if library
		.db
		.location()
		.count(vec![location::path::equals(Some(path.clone()))])
		.exec()
		.await? > 0
	{
		return Err(LocationError::LocationAlreadyExists(
			location_path.as_ref().to_path_buf(),
		));
	}

	if check_nested_location(&location_path, &library.db).await? {
		return Err(LocationError::NestedLocation(
			location_path.as_ref().to_path_buf(),
		));
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
								pub_id: uuid_to_bytes(library.sync.instance)
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
							location::instance_id::set(Some(library.config.instance_id)),
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
	node.locations.remove(location_id, library.clone()).await?;

	delete_directory(library, location_id, None).await?;

	library
		.db
		.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location_id,
		)])
		.exec()
		.await?;

	let location = library
		.db
		.location()
		.delete(location::id::equals(location_id))
		.exec()
		.await?;

	// TODO: This should really be queued to the proper node so it will always run
	// TODO: Deal with whether a location is online or not
	// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
	if location.instance_id == Some(library.config.instance_id) {
		if let Some(path) = &location.path {
			if let Ok(Some(mut metadata)) = SpacedriveLocationMetadataFile::try_load(path).await {
				metadata.remove_library(library.id).await?;
			}
		}
	}

	invalidate_query!(library, "locations.list");

	info!("Location {} deleted", location_id);

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

pub(super) async fn generate_thumbnail(
	extension: &str,
	cas_id: &str,
	path: impl AsRef<Path>,
	node: &Arc<Node>,
) {
	let path = path.as_ref();
	let output_path = get_thumbnail_path(node, cas_id);

	if let Err(e) = fs::metadata(&output_path).await {
		if e.kind() != io::ErrorKind::NotFound {
			error!(
				"Failed to check if thumbnail exists, but we will try to generate it anyway: {e}"
			);
		}
	// Otherwise we good, thumbnail doesn't exist so we can generate it
	} else {
		debug!(
			"Skipping thumbnail generation for {} because it already exists",
			path.display()
		);
		return;
	}

	if let Ok(extension) = ImageExtension::from_str(extension) {
		if can_generate_thumbnail_for_image(&extension) {
			if let Err(e) = generate_image_thumbnail(path, &output_path).await {
				error!("Failed to image thumbnail on location manager: {e:#?}");
			}
		}
	}

	#[cfg(feature = "ffmpeg")]
	{
		use crate::object::preview::{can_generate_thumbnail_for_video, generate_video_thumbnail};
		use sd_file_ext::extensions::VideoExtension;

		if let Ok(extension) = VideoExtension::from_str(extension) {
			if can_generate_thumbnail_for_video(&extension) {
				if let Err(e) = generate_video_thumbnail(path, &output_path).await {
					error!("Failed to video thumbnail on location manager: {e:#?}");
				}
			}
		}
	}

	trace!("Emitting new thumbnail event");
	node.emit(CoreEvent::NewThumbnail {
		thumb_key: get_thumb_key(cas_id),
	});
}
