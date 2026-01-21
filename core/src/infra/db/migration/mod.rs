//! Database migrations

use sea_orm_migration::prelude::*;

mod m20240101_000001_initial_schema;
mod m20240102_000001_populate_lookups;
mod m20240107_000001_create_collections;
mod m20250103_000001_migrate_space_item_entry_id_to_uuid;
mod m20250109_000001_create_sidecars;
mod m20250110_000001_refactor_volumes_table;
mod m20250111_000001_create_spaces;
mod m20250112_000001_create_indexer_rules;
mod m20250115_000001_semantic_tags;
mod m20250120_000001_create_fts5_search_index;
mod m20251009_000001_add_sync_to_devices;
mod m20251015_000001_add_device_slug;
mod m20251015_000002_create_sync_tables;
mod m20251016_000001_add_cloud_identifier;
mod m20251019_000001_add_sync_to_m2m_tables;
mod m20251021_000001_add_indexed_at_to_entries;
mod m20251023_000001_create_device_state_tombstones;
mod m20251102_000001_add_uuid_to_sidecars;
mod m20251109_000001_add_new_content_kinds;
mod m20251112_000001_create_media_data_tables;
mod m20251113_000001_add_job_policies_to_locations;
mod m20251114_000001_add_unique_constraint_to_volumes_uuid;
mod m20251117_000001_add_blurhash_to_media_data;
mod m20251117_000002_add_unique_constraint_to_entries;
mod m20251117_000003_add_unique_bytes_to_volumes;
mod m20251129_000001_add_entry_id_to_space_items;
mod m20251202_000001_add_cloud_config_to_volumes;
mod m20251204_000001_create_cloud_credentials_table;
mod m20251209_000001_add_indexing_stats_to_volumes;
mod m20251216_000001_add_device_hardware_specs;
mod m20251220_000001_add_file_count_to_content_kinds;
mod m20251226_000001_add_device_id_to_entries;
mod m20260104_000001_replace_device_id_with_volume_id;
mod m20260105_000001_add_volume_id_to_locations;
mod m20260114_000001_fix_search_index_include_directories;

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
			Box::new(m20251023_000001_create_device_state_tombstones::Migration),
			Box::new(m20251102_000001_add_uuid_to_sidecars::Migration),
			Box::new(m20251109_000001_add_new_content_kinds::Migration),
			Box::new(m20250111_000001_create_spaces::Migration),
			Box::new(m20251112_000001_create_media_data_tables::Migration),
			Box::new(m20251113_000001_add_job_policies_to_locations::Migration),
			Box::new(m20251114_000001_add_unique_constraint_to_volumes_uuid::Migration),
			Box::new(m20251117_000001_add_blurhash_to_media_data::Migration),
			Box::new(m20251117_000002_add_unique_constraint_to_entries::Migration),
			Box::new(m20251117_000003_add_unique_bytes_to_volumes::Migration),
			Box::new(m20251129_000001_add_entry_id_to_space_items::Migration),
			Box::new(m20251202_000001_add_cloud_config_to_volumes::Migration),
			Box::new(m20251204_000001_create_cloud_credentials_table::Migration),
			Box::new(m20251209_000001_add_indexing_stats_to_volumes::Migration),
			Box::new(m20251216_000001_add_device_hardware_specs::Migration),
			Box::new(m20251220_000001_add_file_count_to_content_kinds::Migration),
			Box::new(m20251226_000001_add_device_id_to_entries::Migration),
			Box::new(m20250103_000001_migrate_space_item_entry_id_to_uuid::Migration),
			Box::new(m20260104_000001_replace_device_id_with_volume_id::Migration),
			Box::new(m20260105_000001_add_volume_id_to_locations::Migration),
			Box::new(m20260114_000001_fix_search_index_include_directories::Migration),
		]
	}
}
