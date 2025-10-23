//! Database migrations

use sea_orm_migration::prelude::*;

mod m20240101_000001_initial_schema;
mod m20240102_000001_populate_lookups;
mod m20240107_000001_create_collections;
mod m20250109_000001_create_sidecars;
mod m20250110_000001_refactor_volumes_table;
mod m20250112_000001_create_indexer_rules;
mod m20250115_000001_semantic_tags;
mod m20250120_000001_create_fts5_search_index;
mod m20251009_000001_add_sync_to_devices;
mod m20251015_000001_add_device_slug;
mod m20251015_000002_create_sync_tables;
mod m20251016_000001_add_cloud_identifier;
mod m20251019_000001_add_sync_to_m2m_tables;
mod m20251021_000001_add_indexed_at_to_entries;
mod m20251022_000001_add_version_to_audit_log;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
	fn migrations() -> Vec<Box<dyn MigrationTrait>> {
		vec![
			Box::new(m20240101_000001_initial_schema::Migration),
			Box::new(m20240102_000001_populate_lookups::Migration),
			Box::new(m20240107_000001_create_collections::Migration),
			Box::new(m20250109_000001_create_sidecars::Migration),
			Box::new(m20250110_000001_refactor_volumes_table::Migration),
			Box::new(m20250112_000001_create_indexer_rules::Migration),
			Box::new(m20250115_000001_semantic_tags::Migration),
			Box::new(m20250120_000001_create_fts5_search_index::Migration),
			Box::new(m20251009_000001_add_sync_to_devices::Migration),
			Box::new(m20251015_000001_add_device_slug::Migration),
			Box::new(m20251015_000002_create_sync_tables::Migration),
			Box::new(m20251016_000001_add_cloud_identifier::Migration),
			Box::new(m20251019_000001_add_sync_to_m2m_tables::Migration),
			Box::new(m20251021_000001_add_indexed_at_to_entries::Migration),
			Box::new(m20251022_000001_add_version_to_audit_log::Migration),
		]
	}
}
