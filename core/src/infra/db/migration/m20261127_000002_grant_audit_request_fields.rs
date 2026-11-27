//! Request fields and aggregated rows for the grant audit.
//!
//! Each `grant_audit` row gains what the request carried and how it ended:
//! the grant's label, the client address, the MCP session, the grep or glob
//! pattern, the bytes returned, the result and the duration. All are nullable,
//! so rows written before this migration keep their meaning.
//!
//! Requests that would otherwise write one row each, such as a flood of
//! unknown tokens or of rate-limited calls, are counted in
//! `grant_audit_tally` instead: one row per kind, subject and minute, so a
//! flood cannot fill the table. The audit is now bounded by age as well as by
//! count, which the index on `at` keeps cheap.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let columns = [
			ColumnDef::new(GrantAudit::GrantLabel).string().to_owned(),
			ColumnDef::new(GrantAudit::Client).string().to_owned(),
			ColumnDef::new(GrantAudit::Session).string().to_owned(),
			ColumnDef::new(GrantAudit::Pattern).string().to_owned(),
			ColumnDef::new(GrantAudit::Bytes).big_integer().to_owned(),
			ColumnDef::new(GrantAudit::Result).string().to_owned(),
			ColumnDef::new(GrantAudit::DurationMs)
				.big_integer()
				.to_owned(),
		];
		for mut column in columns {
			manager
				.alter_table(
					Table::alter()
						.table(GrantAudit::Table)
						.add_column(&mut column)
						.to_owned(),
				)
				.await?;
		}
		manager
			.create_index(
				Index::create()
					.name("idx_grant_audit_at")
					.table(GrantAudit::Table)
					.col(GrantAudit::At)
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(GrantAuditTally::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(GrantAuditTally::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(GrantAuditTally::Kind).string().not_null())
					.col(ColumnDef::new(GrantAuditTally::Subject).string().not_null())
					.col(ColumnDef::new(GrantAuditTally::GrantId).uuid())
					.col(ColumnDef::new(GrantAuditTally::Client).string())
					.col(
						ColumnDef::new(GrantAuditTally::Minute)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(GrantAuditTally::Count)
							.big_integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(GrantAuditTally::LastAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("idx_grant_audit_tally_key")
					.table(GrantAuditTally::Table)
					.col(GrantAuditTally::Kind)
					.col(GrantAuditTally::Subject)
					.col(GrantAuditTally::Minute)
					.unique()
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("idx_grant_audit_tally_minute")
					.table(GrantAuditTally::Table)
					.col(GrantAuditTally::Minute)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(GrantAuditTally::Table).to_owned())
			.await?;
		manager
			.drop_index(
				Index::drop()
					.name("idx_grant_audit_at")
					.table(GrantAudit::Table)
					.to_owned(),
			)
			.await?;
		for column in [
			GrantAudit::GrantLabel,
			GrantAudit::Client,
			GrantAudit::Session,
			GrantAudit::Pattern,
			GrantAudit::Bytes,
			GrantAudit::Result,
			GrantAudit::DurationMs,
		] {
			manager
				.alter_table(
					Table::alter()
						.table(GrantAudit::Table)
						.drop_column(column)
						.to_owned(),
				)
				.await?;
		}
		Ok(())
	}
}

#[derive(DeriveIden)]
enum GrantAudit {
	Table,
	At,
	GrantLabel,
	Client,
	Session,
	Pattern,
	Bytes,
	Result,
	DurationMs,
}

#[derive(DeriveIden)]
enum GrantAuditTally {
	Table,
	Id,
	Kind,
	Subject,
	GrantId,
	Client,
	Minute,
	Count,
	LastAt,
}
