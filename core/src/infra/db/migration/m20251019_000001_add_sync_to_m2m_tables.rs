//! Migration: Add sync support to M2M junction tables
//!
//! Adds sync fields (uuid, version, updated_at) to many-to-many tables
//! to enable cross-device synchronization of relationships.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add sync columns to collection_entry (SQLite requires separate ALTER statements)
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("collection_entry"))
					.add_column(
						ColumnDef::new(Alias::new("uuid"))
							.uuid()
							.null() // Allow NULL temporarily for existing rows
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("collection_entry"))
					.add_column(
						ColumnDef::new(Alias::new("version"))
							.big_integer()
							.not_null()
							.default(1)
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("collection_entry"))
					.add_column(
						ColumnDef::new(Alias::new("updated_at"))
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp())
					)
					.to_owned(),
			)
			.await?;

		// Add unique index on uuid for collection_entry
		manager
			.create_index(
				Index::create()
					.name("idx_collection_entry_uuid")
					.table(Alias::new("collection_entry"))
					.col(Alias::new("uuid"))
					.unique()
					.to_owned(),
			)
			.await?;

		// Add sync columns to user_metadata_tag
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("user_metadata_tag"))
					.add_column(
						ColumnDef::new(Alias::new("uuid"))
							.uuid()
							.not_null()
							.null()  // Allow NULL temporarily
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("user_metadata_tag"))
					.add_column(
						ColumnDef::new(Alias::new("version"))
							.big_integer()
							.not_null()
							.default(1)
					)
					.to_owned(),
			)
			.await?;

		// Add unique index on uuid for user_metadata_tag
		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tag_uuid")
					.table(Alias::new("user_metadata_tag"))
					.col(Alias::new("uuid"))
					.unique()
					.to_owned(),
			)
			.await?;

		// Add sync columns to tag_relationship
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("tag_relationship"))
					.add_column(
						ColumnDef::new(Alias::new("uuid"))
							.uuid()
							.not_null()
							.null()  // Allow NULL temporarily
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("tag_relationship"))
					.add_column(
						ColumnDef::new(Alias::new("version"))
							.big_integer()
							.not_null()
							.default(1)
					)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("tag_relationship"))
					.add_column(
						ColumnDef::new(Alias::new("updated_at"))
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp())
					)
					.to_owned(),
			)
			.await?;

		// Add unique index on uuid for tag_relationship
		manager
			.create_index(
				Index::create()
					.name("idx_tag_relationship_uuid")
					.table(Alias::new("tag_relationship"))
					.col(Alias::new("uuid"))
					.unique()
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop indexes first
		manager
			.drop_index(
				Index::drop()
					.name("idx_collection_entry_uuid")
					.table(Alias::new("collection_entry"))
					.to_owned(),
			)
			.await?;

		manager
			.drop_index(
				Index::drop()
					.name("idx_user_metadata_tag_uuid")
					.table(Alias::new("user_metadata_tag"))
					.to_owned(),
			)
			.await?;

		manager
			.drop_index(
				Index::drop()
					.name("idx_tag_relationship_uuid")
					.table(Alias::new("tag_relationship"))
					.to_owned(),
			)
			.await?;

		// Drop columns from collection_entry
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("collection_entry"))
					.drop_column(Alias::new("uuid"))
					.drop_column(Alias::new("version"))
					.drop_column(Alias::new("updated_at"))
					.to_owned(),
			)
			.await?;

		// Drop columns from user_metadata_tag
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("user_metadata_tag"))
					.drop_column(Alias::new("uuid"))
					.drop_column(Alias::new("version"))
					.to_owned(),
			)
			.await?;

		// Drop columns from tag_relationship
		manager
			.alter_table(
				Table::alter()
					.table(Alias::new("tag_relationship"))
					.drop_column(Alias::new("uuid"))
					.drop_column(Alias::new("version"))
					.drop_column(Alias::new("updated_at"))
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}
