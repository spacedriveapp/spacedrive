//! Add cloud_config column to volumes table for storing cloud service configuration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add cloud_config column to volumes table (JSON text for service-specific config)
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column(ColumnDef::new(Volumes::CloudConfig).text().null())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove cloud_config column
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.drop_column(Volumes::CloudConfig)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Volumes {
	Table,
	CloudConfig,
}
