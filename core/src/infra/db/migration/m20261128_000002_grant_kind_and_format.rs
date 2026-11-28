//! Grant kinds, token formats and the authority epoch.
//!
//! A standing grant lives up to 30 days on a workspace space; a task grant
//! keeps its 5 minute to 12 hour bounds. A signed token carries its own
//! claims so a node other than the minting one can check it. Grants made
//! before this are task grants with opaque tokens, minted under epoch 0.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for column in [
			ColumnDef::new(AgentGrant::Kind)
				.string()
				.not_null()
				.default("task")
				.to_owned(),
			ColumnDef::new(AgentGrant::TokenFormat)
				.string()
				.not_null()
				.default("opaque")
				.to_owned(),
			ColumnDef::new(AgentGrant::Epoch)
				.big_integer()
				.not_null()
				.default(0)
				.to_owned(),
		] {
			manager
				.alter_table(
					Table::alter()
						.table(AgentGrant::Table)
						.add_column(column)
						.to_owned(),
				)
				.await?;
		}
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for column in [AgentGrant::Kind, AgentGrant::TokenFormat, AgentGrant::Epoch] {
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
	Kind,
	TokenFormat,
	Epoch,
}
