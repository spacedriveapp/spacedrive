//! Migration to add device_id column to entries table
//!
//! This denormalizes the device relationship onto entries for efficient queries.
//! Previously, determining which device an entry belonged to required traversing
//! the parent chain to find the location root, then looking up the device.
//! With device_id directly on entries, it's a simple join.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// 1. Add the device_id column (nullable to support entries without device)
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(ColumnDef::new(Entries::DeviceId).integer())
					.to_owned(),
			)
			.await?;

		// 2. Add index for efficient joins
		manager
			.create_index(
				Index::create()
					.name("idx_entries_device_id")
					.table(Entries::Table)
					.col(Entries::DeviceId)
					.to_owned(),
			)
			.await?;

		// 3. Backfill existing entries by traversing to location roots
		// This finds the ancestor chain for each entry, identifies the root (parent_id IS NULL),
		// then looks up which location has that root as entry_id to get the device_id
		let db = manager.get_connection();
		db.execute_unprepared(
			r#"
			UPDATE entries SET device_id = (
				SELECT l.device_id 
				FROM locations l
				WHERE l.entry_id IN (
					WITH RECURSIVE ancestors AS (
						SELECT id, parent_id FROM entries e2 WHERE e2.id = entries.id
						UNION ALL
						SELECT e.id, e.parent_id FROM entries e
						INNER JOIN ancestors a ON e.id = a.parent_id
					)
					SELECT id FROM ancestors WHERE parent_id IS NULL
				)
				LIMIT 1
			)
			WHERE device_id IS NULL
			"#,
		)
		.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop index first
		manager
			.drop_index(
				Index::drop()
					.name("idx_entries_device_id")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		// Drop column
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.drop_column(Entries::DeviceId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Entries {
	Table,
	DeviceId,
}
