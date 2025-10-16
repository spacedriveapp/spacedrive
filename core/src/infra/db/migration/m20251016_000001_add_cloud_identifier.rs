use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add cloud_identifier column to volumes table
		// This stores the actual cloud resource identifier (bucket/drive/container name)
		// separately from mount_point to handle duplicate resource names
		manager
			.alter_table(
				Table::alter()
					.table(Volumes::Table)
					.add_column_if_not_exists(ColumnDef::new(Volumes::CloudIdentifier).string())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove cloud_identifier column
		// Note: SQLite doesn't support dropping columns easily
		Ok(())
	}
}

#[derive(DeriveIden)]
enum Volumes {
	Table,
	CloudIdentifier,
}
