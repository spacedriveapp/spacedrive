use std::path::PathBuf;

use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::node;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uuid::Uuid;

use crate::{
	prisma::{indexer_rule, PrismaClient},
	util::{
		db::uuid_to_bytes,
		migrator::{Migrate, MigratorError},
	},
};

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct LibraryConfig {
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: Option<String>,
	/// P2P identity of this library.
	pub identity: Vec<u8>,
	/// Id of the current node
	pub node_id: Vec<u8>,
	// /// is_encrypted is a flag that is set to true if the library is encrypted.
	// #[serde(default)]
	// pub is_encrypted: bool,
}

impl LibraryConfig {
	pub fn new(name: String) -> Self {
		Self {
			name,
			description: None,
			identity: Identity::new().to_bytes().to_vec(),
			node_id: Uuid::new_v4().as_bytes().to_vec(),
		}
	}
}

#[async_trait::async_trait]
impl Migrate for LibraryConfig {
	const CURRENT_VERSION: u32 = 3;

	type Ctx = PrismaClient;

	fn default(path: PathBuf) -> Result<Self, MigratorError> {
		Err(MigratorError::ConfigFileMissing(path))
	}

	async fn migrate(
		to_version: u32,
		config: &mut serde_json::Map<String, serde_json::Value>,
		db: &Self::Ctx,
	) -> Result<(), MigratorError> {
		match to_version {
			0 => {}
			1 => {
				let rules = vec![
					format!("No OS protected"),
					format!("No Hidden"),
					format!("Only Git Repositories"),
					format!("Only Images"),
				];

				db._batch(
					rules
						.into_iter()
						.enumerate()
						.map(|(i, name)| {
							db.indexer_rule().update_many(
								vec![indexer_rule::name::equals(name)],
								vec![indexer_rule::pub_id::set(Some(uuid_to_bytes(
									Uuid::from_u128(i as u128),
								)))],
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
			3 => {
				// The fact I have to migrate this hurts my soul
				if db.node().count(vec![]).exec().await.unwrap() != 0 {
					panic!(
						"Ummm, there are too many nodes in the database, this should not happen!"
					);
				}

				let new_not_cringe_node_id = Uuid::new_v4();
				db.node()
					.update_many(
						vec![],
						vec![node::pub_id::set(
							new_not_cringe_node_id.as_bytes().to_vec(),
						)],
					)
					.exec()
					.await?;

				config.insert(
					"node_id".into(),
					Value::String(new_not_cringe_node_id.to_string()),
				);
			}
			v => unreachable!("Missing migration for library version {}", v),
		}

		Ok(())
	}
}

// used to return to the frontend with uuid context
#[derive(Serialize, Deserialize, Debug, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub config: LibraryConfig,
}
