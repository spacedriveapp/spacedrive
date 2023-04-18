use serde_json::{Map, Value};

use crate::util::migrator::MigratorError;

pub(crate) const NODE_VERSION: u32 = 0;
pub(crate) const LIBRARY_VERSION: u32 = 0;

/// Used to run migrations at a node level. This is useful for breaking changes to the `NodeConfig` file.
pub fn migration_node(version: u32, _config: &mut Map<String, Value>) -> Result<(), MigratorError> {
	match version {
		0 => Ok(()),
		v => unreachable!("Missing migration for library version {}", v),
	}
}

/// Used to run migrations at a library level. This will be run for every library as necessary.
pub fn migration_library(
	version: u32,
	_config: &mut Map<String, Value>,
) -> Result<(), MigratorError> {
	match version {
		0 => Ok(()),
		v => unreachable!("Missing migration for library version {}", v),
	}
}
