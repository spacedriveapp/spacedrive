//! Create cloud_credentials table for storing encrypted cloud service credentials

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(CloudCredentials::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(CloudCredentials::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(CloudCredentials::VolumeFingerprint)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(CloudCredentials::EncryptedCredential)
							.binary()
							.not_null(),
					)
					.col(
						ColumnDef::new(CloudCredentials::ServiceType)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(CloudCredentials::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(CloudCredentials::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create index on volume_fingerprint for fast lookups
		manager
			.create_index(
				Index::create()
					.name("idx_cloud_credentials_volume_fingerprint")
					.table(CloudCredentials::Table)
					.col(CloudCredentials::VolumeFingerprint)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(CloudCredentials::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum CloudCredentials {
	Table,
	Id,
	VolumeFingerprint,
	EncryptedCredential,
	ServiceType,
	CreatedAt,
	UpdatedAt,
}
