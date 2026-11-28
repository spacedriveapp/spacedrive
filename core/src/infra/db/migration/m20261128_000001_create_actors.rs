//! Actors, and the space and actor a grant belongs to.
//!
//! Every change in a space names who made it: a person, a device, or an agent.
//! An agent is one actor however many grants it holds, so its history follows
//! it from task to task. Grants made before this have neither column set.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(Actor::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Actor::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Actor::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Actor::Kind).string().not_null())
					.col(ColumnDef::new(Actor::Label).string().not_null())
					.col(ColumnDef::new(Actor::Harness).string())
					.col(ColumnDef::new(Actor::CreatedBy).uuid())
					.col(ColumnDef::new(Actor::CreatedAt).timestamp().not_null())
					.to_owned(),
			)
			.await?;
		for column in [AgentGrant::SpaceId, AgentGrant::ActorId] {
			manager
				.alter_table(
					Table::alter()
						.table(AgentGrant::Table)
						.add_column(ColumnDef::new(column).uuid())
						.to_owned(),
				)
				.await?;
		}
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		for column in [AgentGrant::SpaceId, AgentGrant::ActorId] {
			manager
				.alter_table(
					Table::alter()
						.table(AgentGrant::Table)
						.drop_column(column)
						.to_owned(),
				)
				.await?;
		}
		manager
			.drop_table(Table::drop().table(Actor::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum Actor {
	Table,
	Id,
	Uuid,
	Kind,
	Label,
	Harness,
	CreatedBy,
	CreatedAt,
}

#[derive(DeriveIden)]
enum AgentGrant {
	Table,
	SpaceId,
	ActorId,
}
