use std::{pin::Pin, sync::Arc};

use crate::{
	prisma::{indexer_rule, PrismaClient},
	util::db::uuid_to_bytes,
};
use futures::Future;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::util::migrator::MigratorError;

pub(crate) const NODE_VERSION: u32 = 0;
pub(crate) const LIBRARY_VERSION: u32 = 1;

/// Used to run migrations at a node level. This is useful for breaking changes to the `NodeConfig` file.
pub fn migration_node(
	version: u32,
	_config: &mut Map<String, Value>,
	_: (),
) -> Pin<Box<dyn Future<Output = Result<(), MigratorError>> + Send>> {
	Box::pin(async move {
		match version {
			0 => Ok(()),
			v => unreachable!("Missing migration for library version {}", v),
		}
	})
}

/// Used to run migrations at a library level. This will be run for every library as necessary.
pub fn migration_library(
	version: u32,
	_config: &mut Map<String, Value>,
	db: Arc<PrismaClient>,
) -> Pin<Box<dyn Future<Output = Result<(), MigratorError>> + Send>> {
	Box::pin(async move {
		match version {
			0 => Ok(()),
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

				Ok(())
			}
			v => unreachable!("Missing migration for library version {}", v),
		}
	})
}
