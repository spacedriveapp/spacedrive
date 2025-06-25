//! Initial migration to create all tables

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Libraries table removed - library metadata is stored in library.json

		// Create devices table with hybrid ID system
		manager
			.create_table(
				Table::create()
					.table(Devices::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Devices::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Devices::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Devices::Name).string().not_null())
					.col(ColumnDef::new(Devices::Os).string().not_null())
					.col(ColumnDef::new(Devices::OsVersion).string())
					.col(ColumnDef::new(Devices::HardwareModel).string())
					.col(ColumnDef::new(Devices::NetworkAddresses).json().not_null())
					.col(
						ColumnDef::new(Devices::IsOnline)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(Devices::LastSeenAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(Devices::Capabilities).json().not_null())
					.col(ColumnDef::new(Devices::SyncLeadership).json().not_null())
					.col(
						ColumnDef::new(Devices::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Devices::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create content_kinds lookup table
		manager
			.create_table(
				Table::create()
					.table(ContentKinds::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ContentKinds::Id)
							.integer()
							.not_null()
							.primary_key(),
					)
					.col(
						ColumnDef::new(ContentKinds::Name)
							.string()
							.not_null()
							.unique_key(),
					)
					.to_owned(),
			)
			.await?;

		// Populate content_kinds table with enum values
		let content_kinds = vec![
			(0, "unknown"),
			(1, "image"),
			(2, "video"),
			(3, "audio"),
			(4, "document"),
			(5, "archive"),
			(6, "code"),
			(7, "text"),
			(8, "database"),
			(9, "book"),
			(10, "font"),
			(11, "mesh"),
			(12, "config"),
			(13, "encrypted"),
			(14, "key"),
			(15, "executable"),
			(16, "binary"),
		];

		for (id, name) in content_kinds {
			manager
				.exec_stmt(
					Query::insert()
						.into_table(ContentKinds::Table)
						.columns([ContentKinds::Id, ContentKinds::Name])
						.values_panic([id.into(), name.into()])
						.to_owned(),
				)
				.await?;
		}

		// Create mime_types table for runtime discovered MIME types
		manager
			.create_table(
				Table::create()
					.table(MimeTypes::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(MimeTypes::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(MimeTypes::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(MimeTypes::MimeType)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(MimeTypes::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create locations table with hybrid ID system
		manager
			.create_table(
				Table::create()
					.table(Locations::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Locations::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Locations::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(Locations::DeviceId).integer().not_null())
					.col(ColumnDef::new(Locations::Path).string().not_null())
					.col(ColumnDef::new(Locations::Name).string())
					.col(ColumnDef::new(Locations::IndexMode).string().not_null())
					.col(ColumnDef::new(Locations::ScanState).string().not_null())
					.col(ColumnDef::new(Locations::LastScanAt).timestamp_with_time_zone())
					.col(ColumnDef::new(Locations::ErrorMessage).string())
					.col(
						ColumnDef::new(Locations::TotalFileCount)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Locations::TotalByteSize)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Locations::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Locations::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.from(Locations::Table, Locations::DeviceId)
							.to(Devices::Table, Devices::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create content_identities table with hybrid ID system and optional UUID
		manager
			.create_table(
				Table::create()
					.table(ContentIdentities::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(ContentIdentities::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(ContentIdentities::Uuid).uuid()) // Optional until content identification phase
					.col(ColumnDef::new(ContentIdentities::IntegrityHash).string()) // Full hash for file validation
					.col(
						ColumnDef::new(ContentIdentities::ContentHash)
							.string()
							.not_null(),
					) // Fast sampled hash for deduplication
					.col(ColumnDef::new(ContentIdentities::MimeTypeId).integer())
					.col(
						ColumnDef::new(ContentIdentities::KindId)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(ContentIdentities::MediaData).json())
					.col(ColumnDef::new(ContentIdentities::TextContent).text())
					.col(
						ColumnDef::new(ContentIdentities::TotalSize)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(ContentIdentities::EntryCount)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(ContentIdentities::FirstSeenAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(ContentIdentities::LastVerifiedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.from(ContentIdentities::Table, ContentIdentities::KindId)
							.to(ContentKinds::Table, ContentKinds::Id)
							.on_delete(ForeignKeyAction::Restrict),
					)
					.foreign_key(
						ForeignKey::create()
							.from(ContentIdentities::Table, ContentIdentities::MimeTypeId)
							.to(MimeTypes::Table, MimeTypes::Id)
							.on_delete(ForeignKeyAction::SetNull),
					)
					.to_owned(),
			)
			.await?;

		// Create user_metadata table with hierarchical scoping
		manager
			.create_table(
				Table::create()
					.table(UserMetadata::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserMetadata::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(UserMetadata::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					// Exactly one of these is set - defines the scope
					.col(ColumnDef::new(UserMetadata::EntryUuid).uuid()) // File-specific metadata
					.col(ColumnDef::new(UserMetadata::ContentIdentityUuid).uuid()) // Content-universal metadata
					.col(ColumnDef::new(UserMetadata::Notes).text())
					.col(
						ColumnDef::new(UserMetadata::Favorite)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(UserMetadata::Hidden)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(ColumnDef::new(UserMetadata::CustomData).json().not_null())
					.col(
						ColumnDef::new(UserMetadata::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(UserMetadata::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Add constraint to ensure exactly one scope is set - using raw SQL for SQLite compatibility
		if manager.get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
			// SQLite doesn't support adding constraints to existing tables easily
			// The constraint logic will be enforced at the application level
		} else {
			// For other databases that support ALTER TABLE ADD CONSTRAINT
			manager
				.get_connection()
				.execute_unprepared(
					"ALTER TABLE user_metadata ADD CONSTRAINT check_metadata_scope \
					 CHECK ((entry_uuid IS NOT NULL AND content_identity_uuid IS NULL) OR \
					        (entry_uuid IS NULL AND content_identity_uuid IS NOT NULL))"
				)
				.await?;
		}

		// Note: Foreign key constraints for UserMetadata will be added after entries table is created

		// Create entries table with optimized storage
		manager
			.create_table(
				Table::create()
					.table(Entries::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Entries::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Entries::Uuid).uuid()) // Optional until content identification phase
					.col(ColumnDef::new(Entries::LocationId).integer().not_null())
					.col(ColumnDef::new(Entries::RelativePath).string().not_null())
					.col(ColumnDef::new(Entries::Name).string().not_null())
					.col(ColumnDef::new(Entries::Kind).integer().not_null())
					.col(ColumnDef::new(Entries::Extension).string())
					.col(ColumnDef::new(Entries::MetadataId).uuid()) // References UserMetadata.uuid
					.col(ColumnDef::new(Entries::ContentId).integer())
					.col(ColumnDef::new(Entries::Size).big_integer().not_null())
					.col(
						ColumnDef::new(Entries::AggregateSize)
							.big_integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Entries::ChildCount)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Entries::FileCount)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Entries::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Entries::ModifiedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(Entries::AccessedAt).timestamp_with_time_zone())
					.col(ColumnDef::new(Entries::Permissions).string())
					.col(ColumnDef::new(Entries::Inode).big_integer())
					.foreign_key(
						ForeignKey::create()
							.from(Entries::Table, Entries::LocationId)
							.to(Locations::Table, Locations::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.from(Entries::Table, Entries::MetadataId)
							.to(UserMetadata::Table, UserMetadata::Uuid)
							.on_delete(ForeignKeyAction::SetNull), // Allow NULL during sync resolution
					)
					.foreign_key(
						ForeignKey::create()
							.from(Entries::Table, Entries::ContentId)
							.to(ContentIdentities::Table, ContentIdentities::Id)
							.on_delete(ForeignKeyAction::SetNull),
					)
					.to_owned(),
			)
			.await?;

		// Create tags table with hybrid ID system
		manager
			.create_table(
				Table::create()
					.table(Tags::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Tags::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Tags::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Tags::Name).string().not_null())
					.col(ColumnDef::new(Tags::Color).string())
					.col(ColumnDef::new(Tags::Icon).string())
					.col(
						ColumnDef::new(Tags::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Tags::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create labels table with hybrid ID system
		manager
			.create_table(
				Table::create()
					.table(Labels::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Labels::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Labels::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Labels::Name).string().not_null())
					.col(
						ColumnDef::new(Labels::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create user_metadata_tags junction table (renamed for clarity)
		manager
			.create_table(
				Table::create()
					.table(UserMetadataTags::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(UserMetadataTags::UserMetadataId)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(UserMetadataTags::TagUuid).uuid().not_null())
					.col(
						ColumnDef::new(UserMetadataTags::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(UserMetadataTags::DeviceUuid)
							.uuid()
							.not_null(),
					)
					.primary_key(
						Index::create()
							.col(UserMetadataTags::UserMetadataId)
							.col(UserMetadataTags::TagUuid),
					)
					.foreign_key(
						ForeignKey::create()
							.from(UserMetadataTags::Table, UserMetadataTags::UserMetadataId)
							.to(UserMetadata::Table, UserMetadata::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.from(UserMetadataTags::Table, UserMetadataTags::TagUuid)
							.to(Tags::Table, Tags::Uuid)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.from(UserMetadataTags::Table, UserMetadataTags::DeviceUuid)
							.to(Devices::Table, Devices::Uuid)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create metadata_labels junction table
		manager
			.create_table(
				Table::create()
					.table(MetadataLabels::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(MetadataLabels::MetadataId)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(MetadataLabels::LabelId).integer().not_null())
					.primary_key(
						Index::create()
							.col(MetadataLabels::MetadataId)
							.col(MetadataLabels::LabelId),
					)
					.foreign_key(
						ForeignKey::create()
							.from(MetadataLabels::Table, MetadataLabels::MetadataId)
							.to(UserMetadata::Table, UserMetadata::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.from(MetadataLabels::Table, MetadataLabels::LabelId)
							.to(Labels::Table, Labels::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create indices for better query performance
		manager
			.create_index(
				Index::create()
					.name("idx_entries_location_path")
					.table(Entries::Table)
					.col(Entries::LocationId)
					.col(Entries::RelativePath)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_content_hash")
					.table(ContentIdentities::Table)
					.col(ContentIdentities::ContentHash)
					.unique()
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_locations_path")
					.table(Locations::Table)
					.col(Locations::Path)
					.to_owned(),
			)
			.await?;

		// Create indexes for performance on new fields
		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_entry")
					.table(UserMetadata::Table)
					.col(UserMetadata::EntryUuid)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_content")
					.table(UserMetadata::Table)
					.col(UserMetadata::ContentIdentityUuid)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tags_metadata")
					.table(UserMetadataTags::Table)
					.col(UserMetadataTags::UserMetadataId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tags_tag")
					.table(UserMetadataTags::Table)
					.col(UserMetadataTags::TagUuid)
					.to_owned(),
			)
			.await?;

		// Add unique constraints for optional UUID fields
		manager
			.create_index(
				Index::create()
					.name("idx_content_identities_uuid_unique")
					.table(ContentIdentities::Table)
					.col(ContentIdentities::Uuid)
					.unique()
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_entries_uuid_unique")
					.table(Entries::Table)
					.col(Entries::Uuid)
					.unique()
					.if_not_exists()
					.to_owned(),
			)
			.await?;

		// For SQLite, we skip foreign key constraints since they can't be added later
		// The relationships will be enforced at the application level
		if manager.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
			// Add foreign key constraints for UserMetadata scoping (non-SQLite databases)
			manager
				.get_connection()
				.execute_unprepared(
					"ALTER TABLE user_metadata ADD CONSTRAINT fk_user_metadata_entry \
					 FOREIGN KEY (entry_uuid) REFERENCES entries(uuid) ON DELETE CASCADE"
				)
				.await?;

			manager
				.get_connection()
				.execute_unprepared(
					"ALTER TABLE user_metadata ADD CONSTRAINT fk_user_metadata_content \
					 FOREIGN KEY (content_identity_uuid) REFERENCES content_identities(uuid) ON DELETE CASCADE"
				)
				.await?;
		}

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop tables in reverse order of creation
		manager
			.drop_table(Table::drop().table(MetadataLabels::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(UserMetadataTags::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Labels::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Tags::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Entries::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(UserMetadata::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ContentIdentities::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(MimeTypes::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ContentKinds::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Locations::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Devices::Table).to_owned())
			.await?;
		// Libraries table removed - no need to drop

		Ok(())
	}
}

// Table identifiers
// Libraries enum removed - library metadata stored in library.json

#[derive(Iden)]
enum Devices {
	Table,
	Id,
	Uuid,
	Name,
	Os,
	OsVersion,
	HardwareModel,
	NetworkAddresses,
	IsOnline,
	LastSeenAt,
	Capabilities,
	SyncLeadership,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum ContentKinds {
	Table,
	Id,
	Name,
}

#[derive(Iden)]
enum MimeTypes {
	Table,
	Id,
	Uuid,
	MimeType,
	CreatedAt,
}

#[derive(Iden)]
enum Locations {
	Table,
	Id,
	Uuid,
	DeviceId,
	Path,
	Name,
	IndexMode,
	ScanState,
	LastScanAt,
	ErrorMessage,
	TotalFileCount,
	TotalByteSize,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum ContentIdentities {
	Table,
	Id,
	Uuid,
	IntegrityHash,
	ContentHash,
	MimeTypeId,
	KindId,
	MediaData,
	TextContent,
	TotalSize,
	EntryCount,
	FirstSeenAt,
	LastVerifiedAt,
}

#[derive(Iden)]
enum UserMetadata {
	Table,
	Id,
	Uuid,
	EntryUuid,
	ContentIdentityUuid,
	Notes,
	Favorite,
	Hidden,
	CustomData,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum Entries {
	Table,
	Id,
	Uuid,
	LocationId,
	RelativePath,
	Name,
	Kind,
	Extension,
	MetadataId,
	ContentId,
	Size,
	AggregateSize,
	ChildCount,
	FileCount,
	CreatedAt,
	ModifiedAt,
	AccessedAt,
	Permissions,
	Inode,
}

#[derive(Iden)]
enum Tags {
	Table,
	Id,
	Uuid,
	Name,
	Color,
	Icon,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum Labels {
	Table,
	Id,
	Uuid,
	Name,
	CreatedAt,
}

#[derive(Iden)]
enum UserMetadataTags {
	Table,
	UserMetadataId,
	TagUuid,
	CreatedAt,
	DeviceUuid,
}

#[derive(Iden)]
enum MetadataLabels {
	Table,
	MetadataId,
	LabelId,
}
