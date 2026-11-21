//! OAuth bindings for agent grants.
//!
//! A grant minted through the MCP endpoint's OAuth flow records the client it
//! was issued to and the hash of its current refresh token. The previous
//! refresh hash is kept so a replayed, already rotated refresh token can be
//! recognized as leaked and the grant revoked. Grants made by the owner leave
//! all three columns empty.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for column in [
			AgentGrant::OauthClient,
			AgentGrant::RefreshHash,
			AgentGrant::PreviousRefreshHash,
		] {
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
			.create_index(
				Index::create()
					.name("idx_agent_grant_previous_refresh_hash")
					.table(AgentGrant::Table)
					.col(AgentGrant::PreviousRefreshHash)
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for index in [
			"idx_agent_grant_previous_refresh_hash",
			"idx_agent_grant_refresh_hash",
		] {
			manager
				.drop_index(Index::drop().name(index).table(AgentGrant::Table).to_owned())
				.await?;
		}
		for column in [
			AgentGrant::PreviousRefreshHash,
			AgentGrant::RefreshHash,
			AgentGrant::OauthClient,
		] {
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
	PreviousRefreshHash,
}
