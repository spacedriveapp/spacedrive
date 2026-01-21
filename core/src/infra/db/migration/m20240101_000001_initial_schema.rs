//! Initial database schema for Spacedrive V2
//!
//! This migration creates all the tables needed for the pure hierarchical
//! virtual location model with closure table support.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create devices table
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
					.col(ColumnDef::new(Devices::IsOnline).boolean().not_null())
					.col(
						ColumnDef::new(Devices::LastSeenAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(Devices::Capabilities).json().not_null())
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

		// Create user_metadata table (modern schema for semantic tagging)
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
					.col(ColumnDef::new(UserMetadata::EntryUuid).uuid()) // File-specific metadata (higher priority)
					.col(ColumnDef::new(UserMetadata::ContentIdentityUuid).uuid()) // Content-universal metadata (lower priority)
					// All metadata types benefit from scope flexibility
					.col(ColumnDef::new(UserMetadata::Notes).text())
					.col(
						ColumnDef::new(UserMetadata::Favorite)
							.boolean()
							.default(false),
					)
					.col(
						ColumnDef::new(UserMetadata::Hidden)
							.boolean()
							.default(false),
					)
					.col(ColumnDef::new(UserMetadata::CustomData).json().not_null()) // Arbitrary JSON data
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

		// Create mime_types table (lookup table)
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
					.col(ColumnDef::new(MimeTypes::Uuid).uuid().not_null().unique_key())
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

		// Create content_kinds table (lookup table)
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
					.col(ColumnDef::new(ContentKinds::Name).string().not_null())
					.to_owned(),
			)
			.await?;

		// Create content_identities table
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
					.col(
						ColumnDef::new(ContentIdentities::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(ContentIdentities::IntegrityHash).string())
					.col(
						ColumnDef::new(ContentIdentities::ContentHash)
							.string()
							.not_null()
							.unique_key(),
					)
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
							.not_null(),
					)
					.col(
						ColumnDef::new(ContentIdentities::EntryCount)
							.integer()
							.not_null()
							.default(1),
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
							.from(ContentIdentities::Table, ContentIdentities::MimeTypeId)
							.to(MimeTypes::Table, MimeTypes::Id)
							.on_delete(ForeignKeyAction::SetNull),
					)
					.foreign_key(
						ForeignKey::create()
							.from(ContentIdentities::Table, ContentIdentities::KindId)
							.to(ContentKinds::Table, ContentKinds::Id)
							.on_delete(ForeignKeyAction::Restrict),
					)
					.to_owned(),
			)
			.await?;

		// Create entries table - This is the core of our hierarchical model
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
					.col(ColumnDef::new(Entries::Uuid).uuid())
					.col(ColumnDef::new(Entries::Name).string().not_null())
					.col(ColumnDef::new(Entries::Kind).integer().not_null())
					.col(ColumnDef::new(Entries::Extension).string())
					.col(ColumnDef::new(Entries::MetadataId).integer())
					.col(ColumnDef::new(Entries::ContentId).integer())
					.col(ColumnDef::new(Entries::Size).big_integer().not_null())
					.col(
						ColumnDef::new(Entries::AggregateSize)
							.big_integer()
							.not_null(),
					)
					.col(ColumnDef::new(Entries::ChildCount).integer().not_null())
					.col(ColumnDef::new(Entries::FileCount).integer().not_null())
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
					.col(ColumnDef::new(Entries::ParentId).integer())
					.foreign_key(
						ForeignKey::create()
							.from(Entries::Table, Entries::MetadataId)
							.to(UserMetadata::Table, UserMetadata::Id)
							.on_delete(ForeignKeyAction::SetNull),
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

		// Create entry_closure table for efficient hierarchical queries
		manager
			.create_table(
				Table::create()
					.table(EntryClosure::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(EntryClosure::AncestorId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(EntryClosure::DescendantId)
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(EntryClosure::Depth).integer().not_null())
					.primary_key(
						Index::create()
							.col(EntryClosure::AncestorId)
							.col(EntryClosure::DescendantId),
					)
					.foreign_key(
						ForeignKey::create()
							.from(EntryClosure::Table, EntryClosure::AncestorId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.from(EntryClosure::Table, EntryClosure::DescendantId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create directory_paths table for caching directory paths
		manager
			.create_table(
				Table::create()
					.table(DirectoryPaths::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(DirectoryPaths::EntryId)
							.integer()
							.primary_key(),
					)
					.col(ColumnDef::new(DirectoryPaths::Path).text().not_null())
					.foreign_key(
						ForeignKey::create()
							.from(DirectoryPaths::Table, DirectoryPaths::EntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create locations table - Now points to entries instead of storing paths
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
					.col(ColumnDef::new(Locations::EntryId).integer()) // Nullable to handle circular FK with entries
					.col(ColumnDef::new(Locations::Name).string())
					.col(ColumnDef::new(Locations::IndexMode).string().not_null())
					.col(ColumnDef::new(Locations::ScanState).string().not_null())
					.col(ColumnDef::new(Locations::LastScanAt).timestamp_with_time_zone())
					.col(ColumnDef::new(Locations::ErrorMessage).text())
					.col(
						ColumnDef::new(Locations::TotalFileCount)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Locations::TotalByteSize)
							.big_integer()
							.not_null(),
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
							.on_delete(ForeignKeyAction::Restrict),
					)
					.foreign_key(
						ForeignKey::create()
							.from(Locations::Table, Locations::EntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create volumes table
		manager
			.create_table(
				Table::create()
					.table(Volumes::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Volumes::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Volumes::Uuid).uuid().not_null())
					.col(ColumnDef::new(Volumes::DeviceId).uuid().not_null())
					.col(ColumnDef::new(Volumes::Fingerprint).string().not_null())
					.col(ColumnDef::new(Volumes::MountPoint).string())
					.col(ColumnDef::new(Volumes::TotalCapacity).big_integer())
					.col(ColumnDef::new(Volumes::AvailableCapacity).big_integer())
					.col(ColumnDef::new(Volumes::IsRemovable).boolean())
					.col(ColumnDef::new(Volumes::IsEjectable).boolean())
					.col(ColumnDef::new(Volumes::FileSystem).string())
					.col(ColumnDef::new(Volumes::DisplayName).string())
					.col(
						ColumnDef::new(Volumes::TrackedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Volumes::LastSeenAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Volumes::IsOnline)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(ColumnDef::new(Volumes::ReadSpeedMbps).integer())
					.col(ColumnDef::new(Volumes::WriteSpeedMbps).integer())
					.col(ColumnDef::new(Volumes::LastSpeedTestAt).timestamp_with_time_zone())
					.col(ColumnDef::new(Volumes::IsNetworkDrive).boolean())
					.col(ColumnDef::new(Volumes::DeviceModel).string())
					.col(ColumnDef::new(Volumes::VolumeType).string())
					.col(ColumnDef::new(Volumes::IsUserVisible).boolean())
					.col(ColumnDef::new(Volumes::AutoTrackEligible).boolean())
					.col(
						ColumnDef::new(Volumes::CreatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Volumes::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.from(Volumes::Table, Volumes::DeviceId)
							.to(Devices::Table, Devices::Uuid)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create audit_log table
		manager
			.create_table(
				Table::create()
					.table(AuditLog::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(AuditLog::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(AuditLog::Uuid)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(AuditLog::ActionType).string().not_null())
					.col(ColumnDef::new(AuditLog::ActorDeviceId).string().not_null())
					.col(ColumnDef::new(AuditLog::Targets).string().not_null())
					.col(ColumnDef::new(AuditLog::Status).string().not_null())
					.col(ColumnDef::new(AuditLog::JobId).string())
					.col(
						ColumnDef::new(AuditLog::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(AuditLog::CompletedAt).timestamp_with_time_zone())
					.col(ColumnDef::new(AuditLog::ErrorMessage).string())
					.col(ColumnDef::new(AuditLog::ResultPayload).string())
					.col(
						ColumnDef::new(AuditLog::Version)
							.big_integer()
							.not_null()
							.default(1),
					)
					.to_owned(),
			)
			.await?;

		// Create sync_checkpoints table
		manager
			.create_table(
				Table::create()
					.table(SyncCheckpoints::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SyncCheckpoints::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(SyncCheckpoints::DeviceId)
							.integer()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(SyncCheckpoints::LastSync)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(SyncCheckpoints::SyncData).json())
					.col(
						ColumnDef::new(SyncCheckpoints::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(SyncCheckpoints::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						ForeignKey::create()
							.from(SyncCheckpoints::Table, SyncCheckpoints::DeviceId)
							.to(Devices::Table, Devices::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create indices for better query performance

		// Entry indices
		manager
			.create_index(
				Index::create()
					.name("idx_entries_uuid")
					.table(Entries::Table)
					.col(Entries::Uuid)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_entries_parent_id")
					.table(Entries::Table)
					.col(Entries::ParentId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_entries_kind")
					.table(Entries::Table)
					.col(Entries::Kind)
					.to_owned(),
			)
			.await?;

		// Entry closure indices for efficient queries
		manager
			.create_index(
				Index::create()
					.name("idx_entry_closure_descendant")
					.table(EntryClosure::Table)
					.col(EntryClosure::DescendantId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_entry_closure_ancestor_depth")
					.table(EntryClosure::Table)
					.col(EntryClosure::AncestorId)
					.col(EntryClosure::Depth)
					.to_owned(),
			)
			.await?;

		// Location indices
		manager
			.create_index(
				Index::create()
					.name("idx_locations_entry_id")
					.table(Locations::Table)
					.col(Locations::EntryId)
					.to_owned(),
			)
			.await?;

		// Content identity index
		manager
			.create_index(
				Index::create()
					.name("idx_content_identities_content_hash")
					.table(ContentIdentities::Table)
					.col(ContentIdentities::ContentHash)
					.to_owned(),
			)
			.await?;

		// Volume indices
		manager
			.create_index(
				Index::create()
					.name("idx_volumes_device_fingerprint")
					.table(Volumes::Table)
					.col(Volumes::DeviceId)
					.col(Volumes::Fingerprint)
					.unique()
					.to_owned(),
			)
			.await?;

		// Audit log indices
		manager
			.create_index(
				Index::create()
					.name("idx_audit_log_action_type")
					.table(AuditLog::Table)
					.col(AuditLog::ActionType)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_audit_log_actor_device")
					.table(AuditLog::Table)
					.col(AuditLog::ActorDeviceId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_audit_log_status")
					.table(AuditLog::Table)
					.col(AuditLog::Status)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_audit_log_job_id")
					.table(AuditLog::Table)
					.col(AuditLog::JobId)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop tables in reverse order of creation
		manager
			.drop_table(Table::drop().table(SyncCheckpoints::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(AuditLog::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Volumes::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Locations::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(DirectoryPaths::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(EntryClosure::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Entries::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ContentIdentities::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(ContentKinds::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(MimeTypes::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(UserMetadata::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Devices::Table).to_owned())
			.await?;

		Ok(())
	}
}

// Table identifiers

#[derive(DeriveIden)]
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
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum MimeTypes {
	Table,
	Id,
	Uuid,
	MimeType,
	CreatedAt,
}

#[derive(DeriveIden)]
enum ContentKinds {
	Table,
	Id,
	Name,
}

#[derive(DeriveIden)]
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

#[derive(DeriveIden)]
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

#[derive(DeriveIden)]
enum Entries {
	Table,
	Id,
	Uuid,
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
	ParentId,
}

#[derive(DeriveIden)]
enum EntryClosure {
	Table,
	AncestorId,
	DescendantId,
	Depth,
}

#[derive(DeriveIden)]
enum DirectoryPaths {
	Table,
	EntryId,
	Path,
}

#[derive(DeriveIden)]
enum Locations {
	Table,
	Id,
	Uuid,
	DeviceId,
	EntryId,
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

#[derive(DeriveIden)]
enum Volumes {
	Table,
	Id,
	Uuid,
	DeviceId,
	Fingerprint,
	DisplayName,
	MountPoint,
	TotalCapacity,
	AvailableCapacity,
	IsRemovable,
	IsEjectable,
	FileSystem,
	TrackedAt,
	LastSeenAt,
	IsOnline,
	ReadSpeedMbps,
	WriteSpeedMbps,
	LastSpeedTestAt,
	IsNetworkDrive,
	DeviceModel,
	VolumeType,
	IsUserVisible,
	AutoTrackEligible,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum AuditLog {
	Table,
	Id,
	Uuid,
	ActionType,
	ActorDeviceId,
	Targets,
	Status,
	JobId,
	CreatedAt,
	CompletedAt,
	ErrorMessage,
	ResultPayload,
	Version,
}

#[derive(DeriveIden)]
enum SyncCheckpoints {
	Table,
	Id,
	DeviceId,
	LastSync,
	SyncData,
	CreatedAt,
	UpdatedAt,
}
