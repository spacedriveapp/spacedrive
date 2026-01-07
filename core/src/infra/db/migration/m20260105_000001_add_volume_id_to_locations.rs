//! Migration to add volume_id to locations table
//!
//! This establishes the link between locations and volumes. The volume_id is nullable
//! and resolved lazily at runtime when locations are accessed. This enables efficient
//! portable volume ownership changes - updating a volume's device_id automatically
//! transfers ownership of all locations and entries on that volume.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add volume_id column (nullable for lazy resolution)
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.add_column(ColumnDef::new(Locations::VolumeId).integer())
					.to_owned(),
			)
			.await?;

		// Add index for efficient joins (SQLite doesn't support adding FKs to existing tables)
		manager
			.create_index(
				Index::create()
					.name("idx_locations_volume_id")
					.table(Locations::Table)
					.col(Locations::VolumeId)
					.to_owned(),
			)
			.await?;

		// Note: No backfill - volume_id will be resolved lazily at runtime
		// when locations are accessed. This avoids complex SQL logic and ensures
		// correctness via Rust's volume resolution methods.

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop index
		manager
			.drop_index(
				Index::drop()
					.name("idx_locations_volume_id")
					.table(Locations::Table)
					.to_owned(),
			)
			.await?;

		// Drop column
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.drop_column(Locations::VolumeId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Locations {
	Table,
	VolumeId,
}

#[derive(DeriveIden)]
enum Volumes {
	Table,
	Id,
}
