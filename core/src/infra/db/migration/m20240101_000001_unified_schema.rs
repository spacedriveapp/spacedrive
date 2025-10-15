//! Unified database schema for Spacedrive V2
//!
//! This migration creates all tables needed for Spacedrive including:
//! - Core hierarchical virtual location model with closure table
//! - Collections, sidecars, and indexer rules
//! - Semantic tagging system with FTS5
//! - Full-text search for entries
//! - Lookup tables and initial data

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create libraries table
		manager
			.create_table(
				Table::create()
					.table(Libraries::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Libraries::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Libraries::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(Libraries::Name).string().not_null())
					.col(ColumnDef::new(Libraries::DbVersion).integer().not_null())
					.col(ColumnDef::new(Libraries::SyncId).uuid())
					.col(
						ColumnDef::new(Libraries::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Libraries::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create devices table (includes sync fields from m20251009_000001_add_sync_to_devices)
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
						ColumnDef::new(Devices::SyncEnabled)
							.boolean()
							.not_null()
							.default(true),
					)
					.col(ColumnDef::new(Devices::LastSyncAt).timestamp_with_time_zone())
					.col(ColumnDef::new(Devices::LastStateWatermark).timestamp_with_time_zone())
					.col(ColumnDef::new(Devices::LastSharedWatermark).text())
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
					.col(ColumnDef::new(UserMetadata::EntryUuid).uuid())
					.col(ColumnDef::new(UserMetadata::ContentIdentityUuid).uuid())
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
					.col(ColumnDef::new(MimeTypes::Uuid).uuid().not_null())
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

		// Populate content_kinds table (from m20240102_000001_populate_lookups)
		let insert_kinds = Query::insert()
			.into_table(ContentKinds::Table)
			.columns([ContentKinds::Id, ContentKinds::Name])
			.values_panic([0.into(), "unknown".into()])
			.values_panic([1.into(), "image".into()])
			.values_panic([2.into(), "video".into()])
			.values_panic([3.into(), "audio".into()])
			.values_panic([4.into(), "document".into()])
			.values_panic([5.into(), "archive".into()])
			.values_panic([6.into(), "code".into()])
			.values_panic([7.into(), "text".into()])
			.values_panic([8.into(), "database".into()])
			.values_panic([9.into(), "book".into()])
			.values_panic([10.into(), "font".into()])
			.values_panic([11.into(), "mesh".into()])
			.values_panic([12.into(), "config".into()])
			.values_panic([13.into(), "encrypted".into()])
			.values_panic([14.into(), "key".into()])
			.values_panic([15.into(), "executable".into()])
			.values_panic([16.into(), "binary".into()])
			.to_owned();

		manager.exec_stmt(insert_kinds).await?;

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

		// Create entries table - Core of hierarchical model
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

		// Create locations table
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
					.col(ColumnDef::new(Locations::EntryId).integer().not_null())
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

		// Create volumes table (includes all fields from m20250110_000001_refactor_volumes_table)
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

		// Create collections table (from m20240107_000001_create_collections)
		manager
			.create_table(
				Table::create()
					.table(Collection::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Collection::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Collection::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(Collection::Name).string().not_null())
					.col(ColumnDef::new(Collection::Description).text().null())
					.col(
						ColumnDef::new(Collection::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Collection::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create collection_entries junction table
		manager
			.create_table(
				Table::create()
					.table(CollectionEntry::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(CollectionEntry::CollectionId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(CollectionEntry::EntryId)
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(CollectionEntry::AddedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.primary_key(
						Index::create()
							.col(CollectionEntry::CollectionId)
							.col(CollectionEntry::EntryId),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_collection_entry_collection")
							.from(CollectionEntry::Table, CollectionEntry::CollectionId)
							.to(Collection::Table, Collection::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_collection_entry_entry")
							.from(CollectionEntry::Table, CollectionEntry::EntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create sidecars table (from m20250109_000001_create_sidecars)
		manager
			.create_table(
				Table::create()
					.table(Sidecar::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Sidecar::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Sidecar::ContentUuid).uuid().not_null())
					.col(ColumnDef::new(Sidecar::Kind).string().not_null())
					.col(ColumnDef::new(Sidecar::Variant).string().not_null())
					.col(ColumnDef::new(Sidecar::Format).string().not_null())
					.col(ColumnDef::new(Sidecar::RelPath).string().not_null())
					.col(ColumnDef::new(Sidecar::SourceEntryId).integer().null())
					.col(ColumnDef::new(Sidecar::Size).big_integer().not_null())
					.col(ColumnDef::new(Sidecar::Checksum).string().null())
					.col(
						ColumnDef::new(Sidecar::Status)
							.string()
							.not_null()
							.default("pending"),
					)
					.col(ColumnDef::new(Sidecar::Source).string().null())
					.col(
						ColumnDef::new(Sidecar::Version)
							.integer()
							.not_null()
							.default(1),
					)
					.col(
						ColumnDef::new(Sidecar::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Sidecar::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sidecar_content")
							.from(Sidecar::Table, Sidecar::ContentUuid)
							.to(ContentIdentities::Table, ContentIdentities::Uuid)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sidecar_source_entry")
							.from(Sidecar::Table, Sidecar::SourceEntryId)
							.to(Entries::Table, Entries::Id)
							.on_delete(ForeignKeyAction::SetNull)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create unique index on (content_uuid, kind, variant)
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_sidecar_unique")
					.table(Sidecar::Table)
					.col(Sidecar::ContentUuid)
					.col(Sidecar::Kind)
					.col(Sidecar::Variant)
					.unique()
					.to_owned(),
			)
			.await?;

		// Create sidecar_availability table
		manager
			.create_table(
				Table::create()
					.table(SidecarAvailability::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SidecarAvailability::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::ContentUuid)
							.uuid()
							.not_null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::Kind)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::Variant)
							.string()
							.not_null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::DeviceUuid)
							.uuid()
							.not_null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::Has)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(SidecarAvailability::Size)
							.big_integer()
							.null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::Checksum)
							.string()
							.null(),
					)
					.col(
						ColumnDef::new(SidecarAvailability::LastSeenAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sidecar_availability_content")
							.from(SidecarAvailability::Table, SidecarAvailability::ContentUuid)
							.to(ContentIdentities::Table, ContentIdentities::Uuid)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_sidecar_availability_device")
							.from(SidecarAvailability::Table, SidecarAvailability::DeviceUuid)
							.to(Devices::Table, Devices::Uuid)
							.on_delete(ForeignKeyAction::Cascade)
							.on_update(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create unique index on (content_uuid, kind, variant, device_uuid)
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_sidecar_availability_unique")
					.table(SidecarAvailability::Table)
					.col(SidecarAvailability::ContentUuid)
					.col(SidecarAvailability::Kind)
					.col(SidecarAvailability::Variant)
					.col(SidecarAvailability::DeviceUuid)
					.unique()
					.to_owned(),
			)
			.await?;

		// Create indexer_rules table (from m20250112_000001_create_indexer_rules)
		manager
			.create_table(
				Table::create()
					.table(IndexerRules::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(IndexerRules::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(IndexerRules::Name)
							.string()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(IndexerRules::Default).boolean().not_null())
					.col(ColumnDef::new(IndexerRules::RulesBlob).binary().not_null())
					.col(
						ColumnDef::new(IndexerRules::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(IndexerRules::UpdatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Create semantic tags tables (from m20250115_000001_semantic_tags)

		// Create the enhanced tag table
		manager
			.create_table(
				Table::create()
					.table(Alias::new("tag"))
					.if_not_exists()
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Alias::new("uuid"))
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(
						ColumnDef::new(Alias::new("canonical_name"))
							.string()
							.not_null(),
					)
					.col(ColumnDef::new(Alias::new("display_name")).string())
					.col(ColumnDef::new(Alias::new("formal_name")).string())
					.col(ColumnDef::new(Alias::new("abbreviation")).string())
					.col(ColumnDef::new(Alias::new("aliases")).json())
					.col(ColumnDef::new(Alias::new("namespace")).string())
					.col(
						ColumnDef::new(Alias::new("tag_type"))
							.string()
							.not_null()
							.default("standard"),
					)
					.col(ColumnDef::new(Alias::new("color")).string())
					.col(ColumnDef::new(Alias::new("icon")).string())
					.col(ColumnDef::new(Alias::new("description")).text())
					.col(
						ColumnDef::new(Alias::new("is_organizational_anchor"))
							.boolean()
							.default(false),
					)
					.col(
						ColumnDef::new(Alias::new("privacy_level"))
							.string()
							.default("normal"),
					)
					.col(
						ColumnDef::new(Alias::new("search_weight"))
							.integer()
							.default(100),
					)
					.col(ColumnDef::new(Alias::new("attributes")).json())
					.col(ColumnDef::new(Alias::new("composition_rules")).json())
					.col(
						ColumnDef::new(Alias::new("created_at"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("updated_at"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(Alias::new("created_by_device")).uuid())
					.to_owned(),
			)
			.await?;

		// Create tag_relationship table
		manager
			.create_table(
				Table::create()
					.table(Alias::new("tag_relationship"))
					.if_not_exists()
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Alias::new("parent_tag_id"))
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("child_tag_id"))
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("relationship_type"))
							.string()
							.not_null()
							.default("parent_child"),
					)
					.col(ColumnDef::new(Alias::new("strength")).float().default(1.0))
					.col(
						ColumnDef::new(Alias::new("created_at"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_relationship_parent")
							.from(Alias::new("tag_relationship"), Alias::new("parent_tag_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_relationship_child")
							.from(Alias::new("tag_relationship"), Alias::new("child_tag_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create tag_closure table for efficient hierarchical queries
		manager
			.create_table(
				Table::create()
					.table(Alias::new("tag_closure"))
					.if_not_exists()
					.col(
						ColumnDef::new(Alias::new("ancestor_id"))
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("descendant_id"))
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(Alias::new("depth")).integer().not_null())
					.col(
						ColumnDef::new(Alias::new("path_strength"))
							.float()
							.not_null(),
					)
					.primary_key(
						Index::create()
							.col(Alias::new("ancestor_id"))
							.col(Alias::new("descendant_id")),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_closure_ancestor")
							.from(Alias::new("tag_closure"), Alias::new("ancestor_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_closure_descendant")
							.from(Alias::new("tag_closure"), Alias::new("descendant_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create user_metadata_tag table
		manager
			.create_table(
				Table::create()
					.table(Alias::new("user_metadata_tag"))
					.if_not_exists()
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(Alias::new("user_metadata_id"))
							.integer()
							.not_null(),
					)
					.col(ColumnDef::new(Alias::new("tag_id")).integer().not_null())
					.col(ColumnDef::new(Alias::new("applied_context")).string())
					.col(ColumnDef::new(Alias::new("applied_variant")).string())
					.col(
						ColumnDef::new(Alias::new("confidence"))
							.float()
							.default(1.0),
					)
					.col(
						ColumnDef::new(Alias::new("source"))
							.string()
							.default("user"),
					)
					.col(ColumnDef::new(Alias::new("instance_attributes")).json())
					.col(
						ColumnDef::new(Alias::new("created_at"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("updated_at"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(ColumnDef::new(Alias::new("device_uuid")).uuid().not_null())
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_user_metadata_tag_metadata")
							.from(
								Alias::new("user_metadata_tag"),
								Alias::new("user_metadata_id"),
							)
							.to(Alias::new("user_metadata"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_user_metadata_tag_tag")
							.from(Alias::new("user_metadata_tag"), Alias::new("tag_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create tag_usage_pattern table
		manager
			.create_table(
				Table::create()
					.table(Alias::new("tag_usage_pattern"))
					.if_not_exists()
					.col(
						ColumnDef::new(Alias::new("id"))
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Alias::new("tag_id")).integer().not_null())
					.col(
						ColumnDef::new(Alias::new("co_occurrence_tag_id"))
							.integer()
							.not_null(),
					)
					.col(
						ColumnDef::new(Alias::new("occurrence_count"))
							.integer()
							.default(1),
					)
					.col(
						ColumnDef::new(Alias::new("last_used_together"))
							.timestamp_with_time_zone()
							.not_null(),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_usage_pattern_tag")
							.from(Alias::new("tag_usage_pattern"), Alias::new("tag_id"))
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						&mut ForeignKey::create()
							.name("fk_tag_usage_pattern_co_occurrence")
							.from(
								Alias::new("tag_usage_pattern"),
								Alias::new("co_occurrence_tag_id"),
							)
							.to(Alias::new("tag"), Alias::new("id"))
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create indices for semantic tags tables

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

		manager
			.create_index(
				Index::create()
					.name("idx_locations_entry_id")
					.table(Locations::Table)
					.col(Locations::EntryId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_content_identities_content_hash")
					.table(ContentIdentities::Table)
					.col(ContentIdentities::ContentHash)
					.to_owned(),
			)
			.await?;

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

		// Collections indices
		manager
			.create_index(
				Index::create()
					.name("idx_collection_name")
					.table(Collection::Table)
					.col(Collection::Name)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_collection_entry_entry_id")
					.table(CollectionEntry::Table)
					.col(CollectionEntry::EntryId)
					.to_owned(),
			)
			.await?;

		// Semantic tags indices
		manager
			.create_index(
				Index::create()
					.name("idx_tag_canonical_name")
					.table(Alias::new("tag"))
					.col(Alias::new("canonical_name"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_namespace")
					.table(Alias::new("tag"))
					.col(Alias::new("namespace"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_type")
					.table(Alias::new("tag"))
					.col(Alias::new("tag_type"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_privacy_level")
					.table(Alias::new("tag"))
					.col(Alias::new("privacy_level"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_relationship_parent")
					.table(Alias::new("tag_relationship"))
					.col(Alias::new("parent_tag_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_relationship_child")
					.table(Alias::new("tag_relationship"))
					.col(Alias::new("child_tag_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_relationship_type")
					.table(Alias::new("tag_relationship"))
					.col(Alias::new("relationship_type"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_closure_ancestor")
					.table(Alias::new("tag_closure"))
					.col(Alias::new("ancestor_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_closure_descendant")
					.table(Alias::new("tag_closure"))
					.col(Alias::new("descendant_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_closure_depth")
					.table(Alias::new("tag_closure"))
					.col(Alias::new("depth"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tag_metadata")
					.table(Alias::new("user_metadata_tag"))
					.col(Alias::new("user_metadata_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tag_tag")
					.table(Alias::new("user_metadata_tag"))
					.col(Alias::new("tag_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_user_metadata_tag_source")
					.table(Alias::new("user_metadata_tag"))
					.col(Alias::new("source"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_usage_pattern_tag")
					.table(Alias::new("tag_usage_pattern"))
					.col(Alias::new("tag_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_usage_pattern_co_occurrence")
					.table(Alias::new("tag_usage_pattern"))
					.col(Alias::new("co_occurrence_tag_id"))
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_tag_fulltext")
					.table(Alias::new("tag"))
					.col(Alias::new("canonical_name"))
					.col(Alias::new("display_name"))
					.col(Alias::new("formal_name"))
					.col(Alias::new("abbreviation"))
					.col(Alias::new("aliases"))
					.col(Alias::new("description"))
					.to_owned(),
			)
			.await?;

		// Create FTS5 virtual table for tag search
		manager
			.get_connection()
			.execute_unprepared(
				"CREATE VIRTUAL TABLE IF NOT EXISTS tag_search_fts USING fts5(
					tag_id UNINDEXED,
					canonical_name,
					display_name,
					formal_name,
					abbreviation,
					aliases,
					description,
					content='tag',
					content_rowid='id'
				)",
			)
			.await?;

		// Create triggers to maintain FTS5 table
		manager
			.get_connection()
			.execute_unprepared(
				"CREATE TRIGGER IF NOT EXISTS tag_ai AFTER INSERT ON tag BEGIN
					INSERT INTO tag_search_fts(
						tag_id, canonical_name, display_name, formal_name,
						abbreviation, aliases, description
					) VALUES (
						NEW.id, NEW.canonical_name, NEW.display_name, NEW.formal_name,
						NEW.abbreviation, NEW.aliases, NEW.description
					);
				END",
			)
			.await?;

		manager
			.get_connection()
			.execute_unprepared(
				"CREATE TRIGGER IF NOT EXISTS tag_au AFTER UPDATE ON tag BEGIN
					UPDATE tag_search_fts SET
						canonical_name = NEW.canonical_name,
						display_name = NEW.display_name,
						formal_name = NEW.formal_name,
						abbreviation = NEW.abbreviation,
						aliases = NEW.aliases,
						description = NEW.description
					WHERE tag_id = NEW.id;
				END",
			)
			.await?;

		manager
			.get_connection()
			.execute_unprepared(
				"CREATE TRIGGER IF NOT EXISTS tag_ad AFTER DELETE ON tag BEGIN
					DELETE FROM tag_search_fts WHERE tag_id = OLD.id;
				END",
			)
			.await?;

		// Create FTS5 search index for entries (from m20250120_000001_create_fts5_search_index)
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE VIRTUAL TABLE search_index USING fts5(
					content='entries',
					content_rowid='id',
					name,
					extension,
					tokenize="unicode61 remove_diacritics 2 tokenchars '.@-_'",
					prefix='2,3'
				);
				"#,
			)
			.await?;

		// Create trigger for INSERT operations
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TRIGGER IF NOT EXISTS entries_search_insert
				AFTER INSERT ON entries WHEN new.kind = 0
				BEGIN
					INSERT INTO search_index(rowid, name, extension)
					VALUES (new.id, new.name, new.extension);
				END;
				"#,
			)
			.await?;

		// Create trigger for UPDATE operations
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TRIGGER IF NOT EXISTS entries_search_update
				AFTER UPDATE ON entries WHEN new.kind = 0
				BEGIN
					UPDATE search_index SET
						name = new.name,
						extension = new.extension
					WHERE rowid = new.id;
				END;
				"#,
			)
			.await?;

		// Create trigger for DELETE operations
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TRIGGER IF NOT EXISTS entries_search_delete
				AFTER DELETE ON entries WHEN old.kind = 0
				BEGIN
					DELETE FROM search_index WHERE rowid = old.id;
				END;
				"#,
			)
			.await?;

		// Populate FTS5 index with existing file entries
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				INSERT INTO search_index(rowid, name, extension)
				SELECT id, name, extension FROM entries WHERE kind = 0;
				"#,
			)
			.await?;

		// Create search analytics table for query optimization
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE TABLE search_analytics (
					id INTEGER PRIMARY KEY AUTOINCREMENT,
					query_text TEXT NOT NULL,
					query_hash TEXT NOT NULL,
					search_mode TEXT NOT NULL,
					execution_time_ms INTEGER NOT NULL,
					result_count INTEGER NOT NULL,
					fts5_used BOOLEAN DEFAULT TRUE,
					semantic_used BOOLEAN DEFAULT FALSE,
					user_clicked_result BOOLEAN DEFAULT FALSE,
					clicked_result_position INTEGER,
					created_at TEXT NOT NULL DEFAULT (datetime('now'))
				);
				"#,
			)
			.await?;

		// Create index on query_hash for performance analytics
		manager
			.get_connection()
			.execute_unprepared(
				r#"
				CREATE INDEX idx_search_analytics_query_hash
				ON search_analytics(query_hash);
				"#,
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop entries FTS5 tables and triggers
		manager
			.get_connection()
			.execute_unprepared("DROP INDEX IF EXISTS idx_search_analytics_query_hash;")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TABLE IF EXISTS search_analytics;")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_delete;")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_update;")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS entries_search_insert;")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TABLE IF EXISTS search_index;")
			.await?;

		// Drop tag FTS5 table and triggers
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS tag_ad")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS tag_au")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TRIGGER IF EXISTS tag_ai")
			.await?;
		manager
			.get_connection()
			.execute_unprepared("DROP TABLE IF EXISTS tag_search_fts")
			.await?;

		// Drop tag tables in reverse order of creation
		manager
			.drop_table(
				Table::drop()
					.table(Alias::new("tag_usage_pattern"))
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(
				Table::drop()
					.table(Alias::new("user_metadata_tag"))
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(Table::drop().table(Alias::new("tag_closure")).to_owned())
			.await?;
		manager
			.drop_table(
				Table::drop()
					.table(Alias::new("tag_relationship"))
					.to_owned(),
			)
			.await?;
		manager
			.drop_table(Table::drop().table(Alias::new("tag")).to_owned())
			.await?;

		// Drop other tables in reverse order of creation
		manager
			.drop_table(Table::drop().table(IndexerRules::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(SidecarAvailability::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Sidecar::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(CollectionEntry::Table).to_owned())
			.await?;
		manager
			.drop_table(Table::drop().table(Collection::Table).to_owned())
			.await?;
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
		manager
			.drop_table(Table::drop().table(Libraries::Table).to_owned())
			.await?;

		Ok(())
	}
}

// Table identifiers

#[derive(DeriveIden)]
enum Libraries {
	Table,
	Id,
	Uuid,
	Name,
	DbVersion,
	SyncId,
	CreatedAt,
	UpdatedAt,
}

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
	SyncEnabled,
	LastSyncAt,
	LastStateWatermark,
	LastSharedWatermark,
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

#[derive(DeriveIden)]
enum Collection {
	Table,
	Id,
	Uuid,
	Name,
	Description,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum CollectionEntry {
	Table,
	CollectionId,
	EntryId,
	AddedAt,
}

#[derive(DeriveIden)]
enum Sidecar {
	Table,
	Id,
	ContentUuid,
	Kind,
	Variant,
	Format,
	RelPath,
	SourceEntryId,
	Size,
	Checksum,
	Status,
	Source,
	Version,
	CreatedAt,
	UpdatedAt,
}

#[derive(DeriveIden)]
enum SidecarAvailability {
	Table,
	Id,
	ContentUuid,
	Kind,
	Variant,
	DeviceUuid,
	Has,
	Size,
	Checksum,
	LastSeenAt,
}

#[derive(DeriveIden)]
enum IndexerRules {
	Table,
	Id,
	Name,
	Default,
	RulesBlob,
	CreatedAt,
	UpdatedAt,
}
