use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add uuid column to sidecars table (required for library sync)
		manager
			.alter_table(
				Table::alter()
					.table(Sidecar::Table)
					.add_column(
						ColumnDef::new(Sidecar::Uuid)
							.uuid()
							.not_null()
							// Temporary default value for existing rows
							.default(Expr::cust("'00000000-0000-0000-0000-000000000000'")),
					)
					.to_owned(),
			)
			.await?;

		// Create unique index on uuid
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_sidecar_uuid")
					.table(Sidecar::Table)
					.col(Sidecar::Uuid)
					.unique()
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove uuid column
		manager
			.alter_table(
				Table::alter()
					.table(Sidecar::Table)
					.drop_column(Sidecar::Uuid)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Sidecar {
	Table,
	Uuid,
}
