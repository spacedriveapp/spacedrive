//! Assertions authored against a source another device owns.
//!
//! A tag write is a finished row rather than a command, so authoring it
//! offline is safe: the row waits here, delivery is an idempotent merge on
//! the owner, and the owner's ack retires it. The outbox lives in the
//! library database because unshipped claims are device-local state; a
//! replica file is a verified artifact and is never written locally.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(AssertionOutbox::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AssertionOutbox::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					// The owner to deliver to.
					.col(
						ColumnDef::new(AssertionOutbox::DeviceUuid)
							.uuid()
							.not_null(),
					)
					.col(
						ColumnDef::new(AssertionOutbox::SourceUuid)
							.uuid()
							.not_null(),
					)
					// `tag` today; overlay writes ride the same table later.
					.col(ColumnDef::new(AssertionOutbox::Kind).string().not_null())
					// A whole merge input, JSON. The rows are final data;
					// delivery replays are absorbed by the assertion primary
					// key on the owner.
					.col(ColumnDef::new(AssertionOutbox::Payload).text().not_null())
					.col(
						ColumnDef::new(AssertionOutbox::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(AssertionOutbox::Attempts)
							.integer()
							.not_null()
							.default(0),
					)
					.col(ColumnDef::new(AssertionOutbox::NextAttemptAt).timestamp_with_time_zone())
					.col(ColumnDef::new(AssertionOutbox::LastError).text())
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(AssertionOutbox::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum AssertionOutbox {
	Table,
	Id,
	DeviceUuid,
	SourceUuid,
	Kind,
	Payload,
	CreatedAt,
	Attempts,
	NextAttemptAt,
	LastError,
}
