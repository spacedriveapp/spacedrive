//! Migration to create location service settings and state tracking tables
//!
//! Creates tables for managing per-location service configuration (watcher, stale detector, sync)
//! and tracking service state (watcher lifecycle, stale detection runs) as specified in INDEX-009.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create location_service_settings table
		manager
			.create_table(
				Table::create()
					.table(LocationServiceSettings::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(LocationServiceSettings::LocationId)
							.integer()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::WatcherEnabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::WatcherConfig)
							.text(),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::StaleDetectorEnabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::StaleDetectorConfig)
							.text(),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::SyncEnabled)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::SyncConfig)
							.text(),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_location_service_settings_location")
							.from(
								LocationServiceSettings::Table,
								LocationServiceSettings::LocationId,
							)
							.to(Location::Table, Location::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create location_watcher_state table
		manager
			.create_table(
				Table::create()
					.table(LocationWatcherState::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(LocationWatcherState::LocationId)
							.integer()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::LastWatchStart)
							.timestamp(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::LastWatchStop)
							.timestamp(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::LastSuccessfulEvent)
							.timestamp(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::WatchInterrupted)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(LocationWatcherState::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_location_watcher_state_location")
							.from(
								LocationWatcherState::Table,
								LocationWatcherState::LocationId,
							)
							.to(Location::Table, Location::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create stale_detection_runs table
		manager
			.create_table(
				Table::create()
					.table(StaleDetectionRuns::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(StaleDetectionRuns::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::LocationId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::JobId)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::TriggeredBy)
							.text()
							.not_null(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::StartedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::CompletedAt)
							.timestamp(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::Status)
							.text()
							.not_null()
							.default("running"),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::DirectoriesPruned)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::DirectoriesScanned)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::ChangesDetected)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::ErrorMessage)
							.text(),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_stale_detection_runs_location")
							.from(
								StaleDetectionRuns::Table,
								StaleDetectionRuns::LocationId,
							)
							.to(Location::Table, Location::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.index(
						Index::create()
							.name("idx_stale_detection_runs_location_id")
							.table(StaleDetectionRuns::Table)
							.col(StaleDetectionRuns::LocationId),
					)
					.index(
						Index::create()
							.name("idx_stale_detection_runs_status")
							.table(StaleDetectionRuns::Table)
							.col(StaleDetectionRuns::Status),
					)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(
				Table::drop()
					.table(StaleDetectionRuns::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;

		manager
			.drop_table(
				Table::drop()
					.table(LocationWatcherState::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;

		manager
			.drop_table(
				Table::drop()
					.table(LocationServiceSettings::Table)
					.if_exists()
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Location {
	Table,
	Id,
}

#[derive(DeriveIden)]
enum LocationServiceSettings {
	Table,
	LocationId,
	WatcherEnabled,
	WatcherConfig,
	StaleDetectorEnabled,
	StaleDetectorConfig,
	SyncEnabled,
	SyncConfig,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum LocationWatcherState {
	Table,
	LocationId,
	LastWatchStart,
	LastWatchStop,
	LastSuccessfulEvent,
	WatchInterrupted,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum StaleDetectionRuns {
	Table,
	Id,
	LocationId,
	JobId,
	TriggeredBy,
	StartedAt,
	CompletedAt,
	Status,
	DirectoriesPruned,
	DirectoriesScanned,
	ChangesDetected,
	ErrorMessage,
}
