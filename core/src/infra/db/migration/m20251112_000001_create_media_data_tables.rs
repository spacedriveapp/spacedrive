//! Create media data tables for image, video, and audio metadata

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create image_media_data table
		manager
			.create_table(
				Table::create()
					.table(ImageMediaData::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ImageMediaData::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(ImageMediaData::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(ImageMediaData::Width).integer().not_null())
					.col(ColumnDef::new(ImageMediaData::Height).integer().not_null())
					.col(ColumnDef::new(ImageMediaData::DateTaken).timestamp().null())
					.col(ColumnDef::new(ImageMediaData::Latitude).double().null())
					.col(ColumnDef::new(ImageMediaData::Longitude).double().null())
					.col(ColumnDef::new(ImageMediaData::CameraMake).string().null())
					.col(ColumnDef::new(ImageMediaData::CameraModel).string().null())
					.col(ColumnDef::new(ImageMediaData::LensModel).string().null())
					.col(ColumnDef::new(ImageMediaData::FocalLength).string().null())
					.col(ColumnDef::new(ImageMediaData::Aperture).string().null())
					.col(ColumnDef::new(ImageMediaData::ShutterSpeed).string().null())
					.col(ColumnDef::new(ImageMediaData::Iso).integer().null())
					.col(ColumnDef::new(ImageMediaData::Orientation).small_integer().null())
					.col(ColumnDef::new(ImageMediaData::ColorSpace).string().null())
					.col(ColumnDef::new(ImageMediaData::ColorProfile).string().null())
					.col(ColumnDef::new(ImageMediaData::BitDepth).string().null())
					.col(ColumnDef::new(ImageMediaData::Artist).string().null())
					.col(ColumnDef::new(ImageMediaData::Copyright).string().null())
					.col(ColumnDef::new(ImageMediaData::Description).text().null())
					.col(
						ColumnDef::new(ImageMediaData::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(ImageMediaData::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create indexes for image_media_data
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_image_media_date_taken")
					.table(ImageMediaData::Table)
					.col(ImageMediaData::DateTaken)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_image_media_dimensions")
					.table(ImageMediaData::Table)
					.col(ImageMediaData::Width)
					.col(ImageMediaData::Height)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_image_media_camera")
					.table(ImageMediaData::Table)
					.col(ImageMediaData::CameraMake)
					.col(ImageMediaData::CameraModel)
					.to_owned(),
			)
			.await?;

		// Create video_media_data table
		manager
			.create_table(
				Table::create()
					.table(VideoMediaData::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(VideoMediaData::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(VideoMediaData::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(VideoMediaData::Width).integer().not_null())
					.col(ColumnDef::new(VideoMediaData::Height).integer().not_null())
					.col(
						ColumnDef::new(VideoMediaData::DurationSeconds)
							.double()
							.null(),
					)
					.col(ColumnDef::new(VideoMediaData::BitRate).big_integer().null())
					.col(ColumnDef::new(VideoMediaData::Codec).string().null())
					.col(ColumnDef::new(VideoMediaData::PixelFormat).string().null())
					.col(ColumnDef::new(VideoMediaData::ColorSpace).string().null())
					.col(ColumnDef::new(VideoMediaData::ColorRange).string().null())
					.col(
						ColumnDef::new(VideoMediaData::ColorPrimaries)
							.string()
							.null(),
					)
					.col(ColumnDef::new(VideoMediaData::ColorTransfer).string().null())
					.col(ColumnDef::new(VideoMediaData::FpsNum).integer().null())
					.col(ColumnDef::new(VideoMediaData::FpsDen).integer().null())
					.col(ColumnDef::new(VideoMediaData::AudioCodec).string().null())
					.col(ColumnDef::new(VideoMediaData::AudioChannels).string().null())
					.col(
						ColumnDef::new(VideoMediaData::AudioSampleRate)
							.integer()
							.null(),
					)
					.col(
						ColumnDef::new(VideoMediaData::AudioBitRate)
							.integer()
							.null(),
					)
					.col(ColumnDef::new(VideoMediaData::Title).string().null())
					.col(ColumnDef::new(VideoMediaData::Artist).string().null())
					.col(ColumnDef::new(VideoMediaData::Album).string().null())
					.col(ColumnDef::new(VideoMediaData::CreationTime).timestamp().null())
					.col(ColumnDef::new(VideoMediaData::DateCaptured).timestamp().null())
					.col(
						ColumnDef::new(VideoMediaData::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(VideoMediaData::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create indexes for video_media_data
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_video_media_duration")
					.table(VideoMediaData::Table)
					.col(VideoMediaData::DurationSeconds)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_video_media_dimensions")
					.table(VideoMediaData::Table)
					.col(VideoMediaData::Width)
					.col(VideoMediaData::Height)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_video_media_codec")
					.table(VideoMediaData::Table)
					.col(VideoMediaData::Codec)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_video_media_date_captured")
					.table(VideoMediaData::Table)
					.col(VideoMediaData::DateCaptured)
					.to_owned(),
			)
			.await?;

		// Create audio_media_data table
		manager
			.create_table(
				Table::create()
					.table(AudioMediaData::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AudioMediaData::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AudioMediaData::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(AudioMediaData::DurationSeconds)
							.double()
							.null(),
					)
					.col(ColumnDef::new(AudioMediaData::BitRate).big_integer().null())
					.col(ColumnDef::new(AudioMediaData::SampleRate).integer().null())
					.col(ColumnDef::new(AudioMediaData::Channels).string().null())
					.col(ColumnDef::new(AudioMediaData::Codec).string().null())
					.col(ColumnDef::new(AudioMediaData::Title).string().null())
					.col(ColumnDef::new(AudioMediaData::Artist).string().null())
					.col(ColumnDef::new(AudioMediaData::Album).string().null())
					.col(ColumnDef::new(AudioMediaData::AlbumArtist).string().null())
					.col(ColumnDef::new(AudioMediaData::Genre).string().null())
					.col(ColumnDef::new(AudioMediaData::Year).integer().null())
					.col(ColumnDef::new(AudioMediaData::TrackNumber).integer().null())
					.col(ColumnDef::new(AudioMediaData::DiscNumber).integer().null())
					.col(ColumnDef::new(AudioMediaData::Composer).string().null())
					.col(ColumnDef::new(AudioMediaData::Publisher).string().null())
					.col(ColumnDef::new(AudioMediaData::Copyright).string().null())
					.col(
						ColumnDef::new(AudioMediaData::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(AudioMediaData::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create indexes for audio_media_data
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_audio_media_artist")
					.table(AudioMediaData::Table)
					.col(AudioMediaData::Artist)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_audio_media_album")
					.table(AudioMediaData::Table)
					.col(AudioMediaData::Album)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_audio_media_duration")
					.table(AudioMediaData::Table)
					.col(AudioMediaData::DurationSeconds)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_audio_media_genre")
					.table(AudioMediaData::Table)
					.col(AudioMediaData::Genre)
					.to_owned(),
			)
			.await?;

		// Add foreign key columns to content_identities (one at a time for SQLite)
		// Note: SQLite doesn't support adding FK constraints to existing tables,
		// so we add columns without constraints
		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.add_column(ColumnDef::new(ContentIdentities::ImageMediaDataId).integer().null())
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.add_column(ColumnDef::new(ContentIdentities::VideoMediaDataId).integer().null())
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.add_column(ColumnDef::new(ContentIdentities::AudioMediaDataId).integer().null())
					.to_owned(),
			)
			.await?;

		// Remove old media_data JSON column
		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.drop_column(ContentIdentities::MediaData)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Add back media_data JSON column
		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.add_column(ColumnDef::new(ContentIdentities::MediaData).json().null())
					.to_owned(),
			)
			.await?;

		// Drop foreign key columns from content_identities (one at a time for SQLite)
		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.drop_column(ContentIdentities::AudioMediaDataId)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.drop_column(ContentIdentities::VideoMediaDataId)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(ContentIdentities::Table)
					.drop_column(ContentIdentities::ImageMediaDataId)
					.to_owned(),
			)
			.await?;

		// Drop media data tables
		manager
			.drop_table(Table::drop().table(AudioMediaData::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(VideoMediaData::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(ImageMediaData::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum ImageMediaData {
	Table,
	Id,
	Uuid,
	Width,
	Height,
	DateTaken,
	Latitude,
	Longitude,
	CameraMake,
	CameraModel,
	LensModel,
	FocalLength,
	Aperture,
	ShutterSpeed,
	Iso,
	Orientation,
	ColorSpace,
	ColorProfile,
	BitDepth,
	Artist,
	Copyright,
	Description,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum VideoMediaData {
	Table,
	Id,
	Uuid,
	Width,
	Height,
	DurationSeconds,
	BitRate,
	Codec,
	PixelFormat,
	ColorSpace,
	ColorRange,
	ColorPrimaries,
	ColorTransfer,
	FpsNum,
	FpsDen,
	AudioCodec,
	AudioChannels,
	AudioSampleRate,
	AudioBitRate,
	Title,
	Artist,
	Album,
	CreationTime,
	DateCaptured,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum AudioMediaData {
	Table,
	Id,
	Uuid,
	DurationSeconds,
	BitRate,
	SampleRate,
	Channels,
	Codec,
	Title,
	Artist,
	Album,
	AlbumArtist,
	Genre,
	Year,
	TrackNumber,
	DiscNumber,
	Composer,
	Publisher,
	Copyright,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum ContentIdentities {
	Table,
	ImageMediaDataId,
	VideoMediaDataId,
	AudioMediaDataId,
	MediaData,
}
