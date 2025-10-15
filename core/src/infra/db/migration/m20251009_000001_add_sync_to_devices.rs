//! Migration to add sync fields to devices table
//!
//! Extends the devices table with sync coordination fields.
//! This eliminates the need for a separate sync_partners table - if a device
//! is registered in a library, it's a sync partner.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add sync_enabled column (defaults to true - all registered devices sync by default)
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(
						ColumnDef::new(Devices::SyncEnabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.to_owned(),
			)
			.await?;

		// Add last_sync_at column to track last successful sync
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::LastSyncAt).timestamp_with_time_zone())
					.to_owned(),
			)
			.await?;

		// Add last_state_watermark column to track last device state watermark
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::LastStateWatermark).timestamp_with_time_zone())
					.to_owned(),
			)
			.await?;

		// Add last_shared_watermark column to track last shared resource watermark (HLC)
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::LastSharedWatermark).string())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::SyncEnabled)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::LastSyncAt)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::LastStateWatermark)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::LastSharedWatermark)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Devices {
	Table,
	SyncEnabled,
	LastSyncAt,
	LastStateWatermark,
	LastSharedWatermark,
}
