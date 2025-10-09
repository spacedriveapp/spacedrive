//! Remove sync_leadership Migration
//!
//! Removes the sync_leadership column from devices table as part of the
//! transition to the leaderless sync architecture.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// SQLite doesn't support DROP COLUMN directly, so we need to:
		// 1. Create new table without sync_leadership
		// 2. Copy data
		// 3. Drop old table
		// 4. Rename new table

		// Create new devices table without sync_leadership
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TABLE devices_new (
					id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
					uuid TEXT NOT NULL UNIQUE,
					name TEXT NOT NULL,
					os TEXT NOT NULL,
					os_version TEXT,
					hardware_model TEXT,
					network_addresses TEXT NOT NULL,
					is_online BOOLEAN NOT NULL DEFAULT 0,
					last_seen_at TEXT NOT NULL,
					capabilities TEXT NOT NULL,
					created_at TEXT NOT NULL,
					updated_at TEXT NOT NULL
				);
				"#,
			)
			.await?;

		// Copy data from old table to new table
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				INSERT INTO devices_new (
					id, uuid, name, os, os_version, hardware_model,
					network_addresses, is_online, last_seen_at, capabilities,
					created_at, updated_at
				)
				SELECT
					id, uuid, name, os, os_version, hardware_model,
					network_addresses, is_online, last_seen_at, capabilities,
					created_at, updated_at
				FROM devices;
				"#,
			)
			.await?;

		// Drop old table
		manager
			.get_connection()
			.execute_unprepared("DROP TABLE devices;")
			.await?;

		// Rename new table to devices
		manager
			.get_connection()
			.execute_unprepared("ALTER TABLE devices_new RENAME TO devices;")
			.await?;

		// Recreate index
		manager
			.get_connection()
			.execute_unprepared("CREATE UNIQUE INDEX idx_devices_uuid ON devices(uuid);")
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Rollback: Add sync_leadership column back
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TABLE devices_new (
					id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
					uuid TEXT NOT NULL UNIQUE,
					name TEXT NOT NULL,
					os TEXT NOT NULL,
					os_version TEXT,
					hardware_model TEXT,
					network_addresses TEXT NOT NULL,
					is_online BOOLEAN NOT NULL DEFAULT 0,
					last_seen_at TEXT NOT NULL,
					capabilities TEXT NOT NULL,
					sync_leadership TEXT NOT NULL DEFAULT '{}',
					created_at TEXT NOT NULL,
					updated_at TEXT NOT NULL
				);
				"#,
			)
			.await?;

		// Copy data back
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				INSERT INTO devices_new (
					id, uuid, name, os, os_version, hardware_model,
					network_addresses, is_online, last_seen_at, capabilities,
					sync_leadership, created_at, updated_at
				)
				SELECT
					id, uuid, name, os, os_version, hardware_model,
					network_addresses, is_online, last_seen_at, capabilities,
					'{}', created_at, updated_at
				FROM devices;
				"#,
			)
			.await?;

		manager
			.get_connection()
			.execute_unprepared("DROP TABLE devices;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("ALTER TABLE devices_new RENAME TO devices;")
			.await?;

		manager
			.get_connection()
			.execute_unprepared("CREATE UNIQUE INDEX idx_devices_uuid ON devices(uuid);")
			.await?;

		Ok(())
	}
}
