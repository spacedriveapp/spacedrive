//! Add blurhash field to image and video media data tables

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add blurhash column to image_media_data
		manager
			.alter_table(
				Table::alter()
					.table(ImageMediaData::Table)
					.add_column(ColumnDef::new(ImageMediaData::Blurhash).string().null())
					.to_owned(),
			)
			.await?;

		// Add blurhash column to video_media_data
		manager
			.alter_table(
				Table::alter()
					.table(VideoMediaData::Table)
					.add_column(ColumnDef::new(VideoMediaData::Blurhash).string().null())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Remove blurhash column from image_media_data
		manager
			.alter_table(
				Table::alter()
					.table(ImageMediaData::Table)
					.drop_column(ImageMediaData::Blurhash)
					.to_owned(),
			)
			.await?;

		// Remove blurhash column from video_media_data
		manager
			.alter_table(
				Table::alter()
					.table(VideoMediaData::Table)
					.drop_column(VideoMediaData::Blurhash)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum ImageMediaData {
	Table,
	Blurhash,
}

#[derive(Iden)]
enum VideoMediaData {
	Table,
	Blurhash,
}

