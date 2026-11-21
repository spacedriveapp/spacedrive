//! OAuth bindings for agent grants.
//!
//! A grant minted through the MCP endpoint's OAuth flow records the client it
//! was issued to and the hash of its current refresh token. Every refresh hash
//! a rotation replaces goes to `agent_grant_spent_refresh`, so a replay of any
//! earlier refresh token, however many rotations ago, is recognized as leaked
//! and revokes the grant. Grants made by the owner leave both columns empty.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for column in [AgentGrant::OauthClient, AgentGrant::RefreshHash] {
			manager
				.alter_table(
					Table::alter()
						.table(AgentGrant::Table)
						.add_column(ColumnDef::new(column).string())
						.to_owned(),
				)
				.await?;
		}
		manager
			.create_index(
				Index::create()
					.name("idx_agent_grant_refresh_hash")
					.table(AgentGrant::Table)
					.col(AgentGrant::RefreshHash)
					.unique()
					.to_owned(),
			)
			.await?;
		manager
			.create_table(
				Table::create()
					.table(AgentGrantSpentRefresh::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AgentGrantSpentRefresh::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AgentGrantSpentRefresh::GrantId)
							.uuid()
							.not_null(),
					)
					.col(
						ColumnDef::new(AgentGrantSpentRefresh::Hash)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(AgentGrantSpentRefresh::SpentAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;
		manager
			.create_index(
				Index::create()
					.name("idx_agent_grant_spent_refresh_grant_id")
					.table(AgentGrantSpentRefresh::Table)
					.col(AgentGrantSpentRefresh::GrantId)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(
				Table::drop()
					.table(AgentGrantSpentRefresh::Table)
					.to_owned(),
			)
			.await?;
		manager
			.drop_index(
				Index::drop()
					.name("idx_agent_grant_refresh_hash")
					.table(AgentGrant::Table)
					.to_owned(),
			)
			.await?;
		for column in [AgentGrant::RefreshHash, AgentGrant::OauthClient] {
			manager
				.alter_table(
					Table::alter()
						.table(AgentGrant::Table)
						.drop_column(column)
						.to_owned(),
				)
				.await?;
		}
		Ok(())
	}
}

#[derive(DeriveIden)]
enum AgentGrant {
	Table,
	OauthClient,
	RefreshHash,
}

#[derive(DeriveIden)]
enum AgentGrantSpentRefresh {
	Table,
	Id,
	GrantId,
	Hash,
	SpentAt,
}
