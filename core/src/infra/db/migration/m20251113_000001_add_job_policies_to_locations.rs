//! Add job_policies column to locations table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.add_column(ColumnDef::new(Locations::JobPolicies).string())
					.to_owned(),
			)
			.await
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Locations::Table)
					.drop_column(Locations::JobPolicies)
					.to_owned(),
			)
			.await
	}
}

#[derive(DeriveIden)]
enum Locations {
	Table,
	JobPolicies,
}
