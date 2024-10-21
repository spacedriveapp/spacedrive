use crate::{
	node::config::NodeConfig,
	util::version_manager::{Kind, ManagedVersion, VersionManager, VersionManagerError},
};

use sd_p2p::{Identity, RemoteIdentity};
use sd_prisma::prisma::{file_path, indexer_rule, instance, location, PrismaClient};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{
	path::{Path, PathBuf},
	sync::{atomic::AtomicBool, Arc},
};

use int_enum::IntEnum;
use prisma_client_rust::not;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use serde_repr::{Deserialize_repr, Serialize_repr};
use specta::Type;
use thiserror::Error;
use tokio::fs;
use tracing::error;
use uuid::Uuid;

use super::name::LibraryName;

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryConfig {
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: LibraryName,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: Option<String>,
	/// id of the current instance so we know who this `.db` is. This can be looked up within the `Instance` table.
	pub instance_id: i32,
	/// cloud_id is the ID of the cloud library this library is linked to.
	/// If this is set we can assume the library is synced with the Cloud.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub cloud_id: Option<String>,
	// false = library is old and sync hasn't been enabled
	// true = sync is enabled as either the library is new or it has been manually toggled on
	#[serde(default)]
	pub generate_sync_operations: Arc<AtomicBool>,
	version: LibraryConfigVersion,

	#[serde(skip, default)]
	pub config_path: PathBuf,
}

#[derive(
	IntEnum,
	Debug,
	Clone,
	Copy,
	Eq,
	PartialEq,
	strum::Display,
	Serialize_repr,
	Deserialize_repr,
	Type,
)]
#[repr(u64)]
pub enum LibraryConfigVersion {
	V0 = 0,
	V1 = 1,
	V2 = 2,
	V3 = 3,
	V4 = 4,
	V5 = 5,
	V6 = 6,
	V7 = 7,
	V8 = 8,
	V9 = 9,
	V10 = 10,
	V11 = 11,
}

impl ManagedVersion<LibraryConfigVersion> for LibraryConfig {
	const LATEST_VERSION: LibraryConfigVersion = LibraryConfigVersion::V11;

	const KIND: Kind = Kind::Json("version");

	type MigrationError = LibraryConfigError;
}

impl LibraryConfig {
	pub(crate) async fn new(
		name: LibraryName,
		description: Option<String>,
		instance_id: i32,
		path: impl AsRef<Path>,
	) -> Result<Self, LibraryConfigError> {
		let this = Self {
			name,
			description,
			instance_id,
			version: Self::LATEST_VERSION,
			cloud_id: None,
			generate_sync_operations: Arc::new(AtomicBool::new(false)),
			config_path: path.as_ref().to_path_buf(),
		};

		this.save(path).await.map(|()| this)
	}

	pub(crate) async fn load(
		path: impl AsRef<Path>,
		_node_config: &NodeConfig,
		db: &PrismaClient,
	) -> Result<Self, LibraryConfigError> {
		let path = path.as_ref();

		let mut loaded_config = VersionManager::<Self, LibraryConfigVersion>::migrate_and_load(
			path,
			|current, next| async move {
				match (current, next) {
					(LibraryConfigVersion::V0, LibraryConfigVersion::V1) => {
						let rules = vec![
							String::from("No OS protected"),
							String::from("No Hidden"),
							String::from("No Git"),
							String::from("Only Images"),
						];

						db._batch(
							rules
								.into_iter()
								.enumerate()
								.map(|(i, name)| {
									db.indexer_rule().update_many(
										vec![indexer_rule::name::equals(Some(name))],
										vec![indexer_rule::pub_id::set(sd_utils::uuid_to_bytes(
											&Uuid::from_u128(i as u128),
										))],
									)
								})
								.collect::<Vec<_>>(),
						)
						.await?;
					}

					(LibraryConfigVersion::V1, LibraryConfigVersion::V2) => {
						let mut config = serde_json::from_slice::<Map<String, Value>>(
							&fs::read(path).await.map_err(|e| {
								VersionManagerError::FileIO(FileIOError::from((path, e)))
							})?,
						)
						.map_err(VersionManagerError::SerdeJson)?;

						config.insert(
							String::from("identity"),
							Value::Array(
								Identity::new()
									.to_bytes()
									.into_iter()
									.map(Into::into)
									.collect(),
							),
						);

						fs::write(
							path,
							&serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?,
						)
						.await
						.map_err(|e| VersionManagerError::FileIO(FileIOError::from((path, e))))?;
					}

					(LibraryConfigVersion::V2, LibraryConfigVersion::V3) => {
						// Removed, can't be automatically updated
						return Err(LibraryConfigError::CriticalUpdateError);
					}

					(LibraryConfigVersion::V3, LibraryConfigVersion::V4) => {
						// -_-
					}

					(LibraryConfigVersion::V4, LibraryConfigVersion::V5) => loop {
						let paths = db
							.file_path()
							.find_many(vec![not![file_path::size_in_bytes::equals(None)]])
							.take(500)
							.select(file_path::select!({ id size_in_bytes }))
							.exec()
							.await?;

						if paths.is_empty() {
							break;
						}

						db._batch(
							paths
								.into_iter()
								.filter_map(|path| {
									maybe_missing(path.size_in_bytes, "file_path.size_in_bytes")
										.map_or_else(
											|e| {
												error!(?e);
												None
											},
											Some,
										)
										.map(|size_in_bytes| {
											let size =
												if let Ok(size) = size_in_bytes.parse::<u64>() {
													Some(size.to_be_bytes().to_vec())
												} else {
													error!(
														file_path_id = %path.id,
														size = %size_in_bytes,
														"File path had invalid size;",
													);

													None
												};

											db.file_path().update(
												file_path::id::equals(path.id),
												vec![
													file_path::size_in_bytes_bytes::set(size),
													file_path::size_in_bytes::set(None),
												],
											)
										})
								})
								.collect::<Vec<_>>(),
						)
						.await?;
					},

					(LibraryConfigVersion::V5, LibraryConfigVersion::V6) => {
						// Removed, can't be automatically updated
						return Err(LibraryConfigError::CriticalUpdateError);
					}

					(LibraryConfigVersion::V6, LibraryConfigVersion::V7) => {
						let instances = db.instance().find_many(vec![]).exec().await?;

						if instances.len() > 1 {
							error!("7 - More than one instance found in the DB... This can't be automatically reconciled!");
							return Err(LibraryConfigError::TooManyInstances);
						}

						let Some(instance) = instances.first() else {
							error!("7 - No instance found... How did you even get this far?!");
							return Err(LibraryConfigError::MissingInstance);
						};

						let mut config = serde_json::from_slice::<Map<String, Value>>(
							&fs::read(path).await.map_err(|e| {
								VersionManagerError::FileIO(FileIOError::from((path, e)))
							})?,
						)
						.map_err(VersionManagerError::SerdeJson)?;

						config.remove("instance_id");
						config.insert(String::from("instance_id"), json!(instance.id));

						fs::write(
							path,
							&serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?,
						)
						.await
						.map_err(|e| VersionManagerError::FileIO(FileIOError::from((path, e))))?;

						// We are relinking all locations to the current instance.
						// If you have more than one node in your database and you're not @Oscar, something went horribly wrong so this is fine.
						db.location()
							.update_many(
								vec![],
								vec![location::instance_id::set(Some(instance.id))],
							)
							.exec()
							.await?;
					}

					(LibraryConfigVersion::V7, LibraryConfigVersion::V8) => {
						let instances = db.device().find_many(vec![]).exec().await?;
						let Some(instance) = instances.first() else {
							error!("8 - No nodes found... How did you even get this far?!");
							return Err(LibraryConfigError::MissingInstance);
						};

						// This should be in 7 but it's added to ensure to hell it runs.
						let mut config = serde_json::from_slice::<Map<String, Value>>(
							&fs::read(path).await.map_err(|e| {
								VersionManagerError::FileIO(FileIOError::from((path, e)))
							})?,
						)
						.map_err(VersionManagerError::SerdeJson)?;

						config.remove("instance_id");
						config.insert(String::from("instance_id"), json!(instance.id));

						fs::write(
							path,
							&serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?,
						)
						.await
						.map_err(|e| VersionManagerError::FileIO(FileIOError::from((path, e))))?;
					}

					(LibraryConfigVersion::V8, LibraryConfigVersion::V9) => {
						db._batch(
							db.instance()
								.find_many(vec![])
								.exec()
								.await?
								.into_iter()
								.map(|i| {
									db.instance().update(
										instance::id::equals(i.id),
										vec![
											// In earlier versions of the app this migration would convert an `Identity` in the `identity` column to a `IdentityOrRemoteIdentity::Identity`.
											// We have removed the `IdentityOrRemoteIdentity` type so we have disabled this change and the V9 -> V10 will take care of it.
											// instance::identity::set(
											// 	// This code is assuming you only have the current node.
											// 	// If you've paired your node with another node, reset your db.
											// 	IdentityOrRemoteIdentity::Identity(
											// 		Identity::from_bytes(&i.identity).expect(
											// 			"Invalid identity detected in DB during migrations",
											// 		),
											// 	)
											// 	.to_bytes(),
											// ),
										],
									)
								})
								.collect::<Vec<_>>(),
						)
						.await?;
					}

					(LibraryConfigVersion::V9, LibraryConfigVersion::V10) => {
						db._batch(
							db.instance()
								.find_many(vec![])
								.exec()
								.await?
								.into_iter()
								.filter_map(|i| {
									let identity = i.identity?;

									let (remote_identity, identity) = if identity[0] == b'I' {
										// We have an `IdentityOrRemoteIdentity::Identity`
										let identity = Identity::from_bytes(&identity[1..]).expect(
											"Invalid identity detected in DB during migrations - 1",
										);

										(identity.to_remote_identity(), Some(identity))
									} else if identity[0] == b'R' {
										// We have an `IdentityOrRemoteIdentity::RemoteIdentity`
										let identity = RemoteIdentity::from_bytes(&identity[1..])
											.expect(
											"Invalid identity detected in DB during migrations - 2",
										);

										(identity, None)
									} else {
										// We have an `Identity` or an invalid column.
										let identity = Identity::from_bytes(&identity).expect(
											"Invalid identity detected in DB during migrations - 3",
										);

										(identity.to_remote_identity(), Some(identity))
									};

									Some(db.instance().update(
										instance::id::equals(i.id),
										vec![
											instance::identity::set(identity.map(|i| i.to_bytes())),
											instance::remote_identity::set(
												remote_identity.get_bytes().to_vec(),
											),
										],
									))
								})
								.collect::<Vec<_>>(),
						)
						.await?;
					}

					(LibraryConfigVersion::V10, LibraryConfigVersion::V11) => {
						db.instance()
							.update_many(
								vec![],
								vec![instance::node_remote_identity::set(Some(
									// This is a remote identity that doesn't exist. The expectation is that:
									// - The current node will update it's own and notice the change causing it to push the updated id to the cloud
									// - All other instances will be updated when the regular sync process with the cloud happens
									"SaEhml9thV088ocsOXZ17BrNjFaROB0ojwBvnPHhztI".into(),
								))],
							)
							.exec()
							.await?;
					}

					_ => {
						error!(current_version = ?current, "Library config version is not handled;");

						return Err(VersionManagerError::UnexpectedMigration {
							current_version: current.int_value(),
							next_version: next.int_value(),
						}
						.into());
					}
				}
				Ok(())
			},
		)
		.await?;

		loaded_config.config_path = path.to_path_buf();

		Ok(loaded_config)
	}

	pub(crate) async fn save(&self, path: impl AsRef<Path>) -> Result<(), LibraryConfigError> {
		let path = path.as_ref();
		fs::write(path, &serde_json::to_vec(self)?)
			.await
			.map_err(|e| FileIOError::from((path, e)).into())
	}
}

#[derive(Error, Debug)]
pub enum LibraryConfigError {
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("there are too many nodes in the database, this should not happen!")]
	TooManyNodes,
	#[error("there are too many instances in the database, this should not happen!")]
	TooManyInstances,
	#[error("missing instances")]
	MissingInstance,
	#[error("your library version can't be automatically updated, please recreate your library")]
	CriticalUpdateError,

	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	#[error(transparent)]
	VersionManager(#[from] VersionManagerError<LibraryConfigVersion>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
