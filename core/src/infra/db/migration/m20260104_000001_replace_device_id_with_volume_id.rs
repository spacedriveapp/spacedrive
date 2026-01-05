//! Migration to replace device_id with volume_id on entries table
//!
//! This changes entries to reference volumes directly instead of devices.
//! When a volume moves between devices (ownership change), only the volume's
//! device_id needs to be updated - all entries inherit the new ownership
//! through the volume relationship. This makes portable volume ownership
//! changes O(1) instead of O(millions).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let db = manager.get_connection();

		// 1. Add the volume_id column (nullable for migration)
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(ColumnDef::new(Entries::VolumeId).integer())
					.to_owned(),
			)
			.await?;

		// 2. Add index for efficient joins (SQLite doesn't support adding FKs to existing tables)
		manager
			.create_index(
				Index::create()
					.name("idx_entries_volume_id")
					.table(Entries::Table)
					.col(Entries::VolumeId)
					.to_owned(),
			)
			.await?;

		// 4. Backfill entries.volume_id by finding each entry's location
		// This traverses the entry tree to find the root, then looks up which
		// location owns that root, then finds which volume that location is on
		// by joining with volumes based on the device_id.
		//
		// Note: This assumes each location is on exactly one volume owned by
		// the location's device. If a device has multiple volumes, we take the
		// first match. In practice, locations should have explicit volume_id
		// references (see follow-up migration for locations).
		db.execute_unprepared(
			r#"
			UPDATE entries SET volume_id = (
				SELECT v.id
				FROM locations l
				INNER JOIN devices d ON d.id = l.device_id
				INNER JOIN volumes v ON v.device_id = d.uuid
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
			WHERE volume_id IS NULL
			"#,
		)
		.await?;

		// 5. Drop the old device_id index
		manager
			.drop_index(
				Index::drop()
					.name("idx_entries_device_id")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		// 6. Drop the device_id column
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

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let db = manager.get_connection();

		// 1. Re-add device_id column
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.add_column(ColumnDef::new(Entries::DeviceId).integer())
					.to_owned(),
			)
			.await?;

		// 2. Re-add device_id index
		manager
			.create_index(
				Index::create()
					.name("idx_entries_device_id")
					.table(Entries::Table)
					.col(Entries::DeviceId)
					.to_owned(),
			)
			.await?;

		// 3. Backfill device_id from volume_id
		db.execute_unprepared(
			r#"
			UPDATE entries SET device_id = (
				SELECT d.id
				FROM volumes v
				INNER JOIN devices d ON d.uuid = v.device_id
				WHERE v.id = entries.volume_id
			)
			WHERE device_id IS NULL AND volume_id IS NOT NULL
			"#,
		)
		.await?;

		// 4. Drop volume_id index
		manager
			.drop_index(
				Index::drop()
					.name("idx_entries_volume_id")
					.table(Entries::Table)
					.to_owned(),
			)
			.await?;

		// 5. Drop volume_id column
		manager
			.alter_table(
				Table::alter()
					.table(Entries::Table)
					.drop_column(Entries::VolumeId)
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
	VolumeId,
}

#[derive(DeriveIden)]
enum Volumes {
	Table,
	Id,
}
