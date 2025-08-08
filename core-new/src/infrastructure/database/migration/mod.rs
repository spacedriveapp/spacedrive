//! Database migrations

use sea_orm_migration::prelude::*;

mod m20240101_000001_initial_schema;
mod m20240107_000001_create_collections;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
	fn migrations() -> Vec<Box<dyn MigrationTrait>> {
		vec![
			Box::new(m20240101_000001_initial_schema::Migration),
			Box::new(m20240107_000001_create_collections::Migration),
		]
	}
}
