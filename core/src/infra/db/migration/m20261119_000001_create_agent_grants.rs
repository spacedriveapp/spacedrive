//! Agent grants and their audit trail.
//!
//! A grant is small shared state that no ingest can rebuild, so it lives in
//! `library.db` beside the source registry it points into. Only the SHA-256 of
//! a grant's token is kept: the token itself is shown once at creation and a
//! copy of this database never yields a usable credential.
//!
//! The audit table is bounded by row count rather than by age, and trimmed on
//! write, so a busy agent cannot grow the library without limit.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(AgentGrant::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AgentGrant::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AgentGrant::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(AgentGrant::TokenHash)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(AgentGrant::SourceId).uuid().not_null())
					.col(ColumnDef::new(AgentGrant::Subtree).string().not_null())
					.col(ColumnDef::new(AgentGrant::Caps).string().not_null())
					.col(ColumnDef::new(AgentGrant::Label).string())
					.col(ColumnDef::new(AgentGrant::CreatedBy).string().not_null())
					.col(
						ColumnDef::new(AgentGrant::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(AgentGrant::ExpiresAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AgentGrant::RevokedAt).timestamp_with_time_zone())
					.to_owned(),
			)
			.await?;

		manager
			.create_table(
				Table::create()
					.table(GrantAudit::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(GrantAudit::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(GrantAudit::GrantId).uuid().not_null())
					.col(ColumnDef::new(GrantAudit::Op).string().not_null())
					.col(ColumnDef::new(GrantAudit::Path).string().not_null())
					.col(ColumnDef::new(GrantAudit::Allowed).boolean().not_null())
					.col(ColumnDef::new(GrantAudit::Reason).string())
					.col(
						ColumnDef::new(GrantAudit::At)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_grant_audit_grant_id")
					.table(GrantAudit::Table)
					.col(GrantAudit::GrantId)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(GrantAudit::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(AgentGrant::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum AgentGrant {
	Table,
	Id,
	Uuid,
	TokenHash,
	SourceId,
	Subtree,
	Caps,
	Label,
	CreatedBy,
	CreatedAt,
	ExpiresAt,
	RevokedAt,
}

#[derive(DeriveIden)]
enum GrantAudit {
	Table,
	Id,
	GrantId,
	Op,
	Path,
	Allowed,
	Reason,
	At,
}
