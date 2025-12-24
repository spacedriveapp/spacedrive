//! Create tables for stale detection service infrastructure
//!
//! Creates three tables:
//! - location_service_settings: Per-location configuration for watcher, stale detector, and sync
//! - location_watcher_state: Tracks filesystem watcher lifecycle for crash recovery
//! - stale_detection_runs: Records history of stale detection runs for monitoring

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
					.col(ColumnDef::new(LocationServiceSettings::WatcherConfig).string())
					.col(
						ColumnDef::new(LocationServiceSettings::StaleDetectorEnabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(ColumnDef::new(LocationServiceSettings::StaleDetectorConfig).string())
					.col(
						ColumnDef::new(LocationServiceSettings::SyncEnabled)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(ColumnDef::new(LocationServiceSettings::SyncConfig).string())
					.col(
						ColumnDef::new(LocationServiceSettings::CreatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(LocationServiceSettings::UpdatedAt)
							.timestamp_with_time_zone()
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
							.to(Locations::Table, Locations::Id)
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
							.timestamp_with_time_zone(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::LastWatchStop)
							.timestamp_with_time_zone(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::LastSuccessfulEvent)
							.timestamp_with_time_zone(),
					)
					.col(
						ColumnDef::new(LocationWatcherState::WatchInterrupted)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(LocationWatcherState::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_location_watcher_state_location")
							.from(LocationWatcherState::Table, LocationWatcherState::LocationId)
							.to(Locations::Table, Locations::Id)
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
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::TriggeredBy)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(StaleDetectionRuns::StartedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(ColumnDef::new(StaleDetectionRuns::CompletedAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(StaleDetectionRuns::Status)
							.string()
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
					.col(ColumnDef::new(StaleDetectionRuns::ErrorMessage).string())
					.foreign_key(
						ForeignKey::create()
							.name("fk_stale_detection_runs_location")
							.from(StaleDetectionRuns::Table, StaleDetectionRuns::LocationId)
							.to(Locations::Table, Locations::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index on stale_detection_runs for efficient queries by location and time
		manager
			.create_index(
				Index::create()
					.name("idx_stale_detection_runs_location_started")
					.table(StaleDetectionRuns::Table)
					.col(StaleDetectionRuns::LocationId)
					.col(StaleDetectionRuns::StartedAt)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop tables in reverse order (dependencies first)
		manager
			.drop_table(Table::drop().table(StaleDetectionRuns::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(LocationWatcherState::Table).to_owned())
			.await?;

		manager
			.drop_table(
				Table::drop()
					.table(LocationServiceSettings::Table)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
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

#[derive(DeriveIden)]
enum Locations {
	Table,
	Id,
}
