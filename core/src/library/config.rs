use serde::{Deserialize, Serialize};
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
#[derive(Debug, Serialize, Deserialize, Clone, Type, Default)]
pub struct LibraryConfig {
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: String,
	// /// is_encrypted is a flag that is set to true if the library is encrypted.
	// #[serde(default)]
	// pub is_encrypted: bool,
}

#[async_trait::async_trait]
impl Migrate for LibraryConfig {
	const CURRENT_VERSION: u32 = 1;

	type Ctx = PrismaClient;

	async fn migrate(
		to_version: u32,
		_config: &mut serde_json::Map<String, serde_json::Value>,
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
