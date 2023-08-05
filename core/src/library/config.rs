use crate::{
	node::{NodeConfig, Platform},
	p2p::IdentityOrRemoteIdentity,
	prisma::{file_path, indexer_rule, PrismaClient},
	util::{
		db::maybe_missing,
		migrator::{Migrate, MigratorError},
	},
};

use chrono::Utc;
use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::{instance, location, node};

use std::{path::PathBuf, sync::Arc};

use prisma_client_rust::not;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
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
}

#[async_trait::async_trait]
impl Migrate for LibraryConfig {
	const CURRENT_VERSION: u32 = 9;

	type Ctx = (NodeConfig, Arc<PrismaClient>);

	fn default(path: PathBuf) -> Result<Self, MigratorError> {
		Err(MigratorError::ConfigFileMissing(path))
	}

	async fn migrate(
		to_version: u32,
		config: &mut serde_json::Map<String, serde_json::Value>,
		(node_config, db): &Self::Ctx,
	) -> Result<(), MigratorError> {
		match to_version {
			0 => {}
			1 => {
				let rules = vec![
					format!("No OS protected"),
					format!("No Hidden"),
					format!("No Git"),
					format!("Only Images"),
				];

				db._batch(
					rules
						.into_iter()
						.enumerate()
						.map(|(i, name)| {
							db.indexer_rule().update_many(
								vec![indexer_rule::name::equals(Some(name))],
								vec![indexer_rule::pub_id::set(sd_utils::uuid_to_bytes(
									Uuid::from_u128(i as u128),
								))],
							)
						})
						.collect::<Vec<_>>(),
				)
				.await?;
			}
			2 => {
				config.insert(
					"identity".into(),
					Value::Array(
						Identity::new()
							.to_bytes()
							.into_iter()
							.map(|v| v.into())
							.collect(),
					),
				);
			}
			// The fact I have to migrate this hurts my soul
			3 => {
				if db.node().count(vec![]).exec().await? != 1 {
					return Err(MigratorError::Custom(
						"Ummm, there are too many nodes in the database, this should not happen!"
							.into(),
					));
				}

				db.node()
					.update_many(
						vec![],
						vec![
							node::pub_id::set(node_config.id.as_bytes().to_vec()),
							node::node_peer_id::set(Some(
								node_config.keypair.peer_id().to_string(),
							)),
						],
					)
					.exec()
					.await?;

				config.insert("node_id".into(), Value::String(node_config.id.to_string()));
			}
			4 => {} // -_-
			5 => loop {
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
										error!("{e:#?}");
										None
									},
									Some,
								)
								.map(|size_in_bytes| {
									let size = if let Ok(size) = size_in_bytes.parse::<u64>() {
										Some(size.to_be_bytes().to_vec())
									} else {
										error!(
											"File path <id='{}'> had invalid size: '{}'",
											path.id, size_in_bytes
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
			6 => {
				let nodes = db.node().find_many(vec![]).exec().await?;

				if nodes.is_empty() {
					println!("6 - No nodes found... How did you even get this far? but this is fine we can fix it.");
				} else if nodes.len() > 1 {
					return Err(MigratorError::Custom(
						"6 - More than one node found in the DB... This can't be automatically reconciled!"
							.into(),
					));
				}

				let node = nodes.first();
				let now = Utc::now().fixed_offset();
				let instance_id = Uuid::new_v4();
				instance::Create {
					pub_id: instance_id.as_bytes().to_vec(),
					identity: node
						.and_then(|n| n.identity.clone())
						.unwrap_or_else(|| Identity::new().to_bytes()),
					node_id: node_config.id.as_bytes().to_vec(),
					node_name: node_config.name.clone(),
					node_platform: Platform::current() as i32,
					last_seen: now,
					date_created: node.map(|n| n.date_created).unwrap_or_else(|| now),
					// timestamp: Default::default(), // TODO: Source this properly!
					_params: vec![],
				}
				.to_query(db)
				.exec()
				.await?;

				config.remove("node_id");
				config.remove("identity");
				config.insert("instance_id".into(), Value::String(instance_id.to_string()));
			}
			7 => {
				let instances = db.instance().find_many(vec![]).exec().await?;

				if instances.len() > 1 {
					return Err(MigratorError::Custom(
						"7 - More than one instance found in the DB... This can't be automatically reconciled!"
							.into(),
					));
				}
				let Some(instance) = instances.first() else {
					return Err(MigratorError::Custom(
						"7 - No nodes found... How did you even get this far?!".into(),
					));
				};

				config.remove("instance_id");
				config.insert("instance_id".into(), Value::Number(instance.id.into()));

				// We are relinking all locations to the current instance.
				// If you have more than one node in your database and your not @Oscar, something went horribly wrong so this is fine.
				db.location()
					.update_many(vec![], vec![location::instance_id::set(Some(instance.id))])
					.exec()
					.await?;
			}
			8 => {
				let instances = db.instance().find_many(vec![]).exec().await?;
				let Some(instance) = instances.first() else {
					return Err(MigratorError::Custom(
						"8 - No nodes found... How did you even get this far?!".into(),
					));
				};

				// This should be in 7 but it's added to ensure to hell it runs.
				config.remove("instance_id");
				config.insert("instance_id".into(), Value::Number(instance.id.into()));
			}
			9 => {
				db._batch(
					db.instance()
						.find_many(vec![])
						.exec()
						.await?
						.into_iter()
						.map(|i| {
							db.instance().update(
								instance::id::equals(i.id),
								vec![instance::identity::set(
									// This code is assuming you only have the current node.
									// If you've paired your node with another node, reset your db.
									IdentityOrRemoteIdentity::Identity(
										Identity::from_bytes(&i.identity).expect(
											"Invalid identity detected in DB during migrations",
										),
									)
									.to_bytes(),
								)],
							)
						})
						.collect::<Vec<_>>(),
				)
				.await?;
			}
			v => unreachable!("Missing migration for library version {}", v),
		}

		Ok(())
	}
}
