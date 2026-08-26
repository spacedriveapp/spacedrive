//! The source registry becomes a table.
//!
//! Until now a source has been two things that never met: `sources.json` beside
//! the per-source directories (filesystem roots, written from one call site)
//! and `registry.db` inside the library's archive directory (adapters). Neither
//! could see the other, and `ops/sources/*` served only the second.
//!
//! One table serves both. `data_type` is what forks them, matching
//! `_schema.data_type_id` in the source's own store: `filesystem` for a walk,
//! the adapter's data type otherwise.
//!
//! ## Medium versus index
//!
//! `volumes` and the old `sources.json` described the same drive twice, joined
//! by a fingerprint string that was a foreign key in neither direction and
//! updated at different moments, so the two counts drifted with nothing to
//! notice. The line that holds is medium against index. A volume is a device
//! fact: capacity, filesystem, speed, removable, online, which machine, true
//! whether or not anything is indexed. A source is a Spacedrive fact: a root, a
//! record count, a snapshot, a restore state.
//!
//! So `volume_uuid` is nullable and lives here rather than a fingerprint. A
//! source is attached when its volume is online and its root resolves under
//! that volume's current mount point, which makes the detached-drive case a
//! property of the medium rather than four scattered calls to `root.exists()`.
//! Nullable because the cardinality is many sources to *optionally* one volume:
//! nested roots register as distinct sources, and adapter, cloud and
//! fingerprint-less network sources have no volume at all.
//!
//! Index facts still on `volumes` (`total_file_count`, `total_directory_count`,
//! `unique_bytes`, `last_indexed_at`) belong on this side of that line. They are
//! left in place here and move once their writers do, so this migration adds a
//! table and takes nothing away.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.create_table(
				Table::create()
					.table(Sources::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Sources::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Sources::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Sources::Name).string().not_null())
					// `filesystem`, or the adapter's data type. Matches
					// `_schema.data_type_id` in the source's own store.
					.col(ColumnDef::new(Sources::DataType).string().not_null())
					// Null for a filesystem source: a walk has no adapter.
					.col(ColumnDef::new(Sources::AdapterId).string())
					.col(
						ColumnDef::new(Sources::Config)
							.text()
							.not_null()
							.default("{}"),
					)
					// The origin. A filesystem source has a root; an adapter
					// source has credentials in `config` and no path at all.
					.col(ColumnDef::new(Sources::Root).string())
					.col(ColumnDef::new(Sources::VolumeUuid).uuid())
					// Index facts. What the source's store holds, as opposed to
					// what the medium underneath it can hold.
					.col(ColumnDef::new(Sources::RecordCount).big_integer())
					.col(ColumnDef::new(Sources::DirectoryCount).big_integer())
					.col(ColumnDef::new(Sources::TotalBytes).big_integer())
					.col(ColumnDef::new(Sources::UniqueBytes).big_integer())
					.col(ColumnDef::new(Sources::LastIndexedAt).timestamp_with_time_zone())
					.col(
						ColumnDef::new(Sources::Status)
							.string()
							.not_null()
							.default("idle"),
					)
					// Travels with the source rather than with its ingest, so
					// screening policy can key on it once screening exists.
					.col(
						ColumnDef::new(Sources::TrustTier)
							.string()
							.not_null()
							.default("external"),
					)
					.col(
						ColumnDef::new(Sources::CreatedAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.col(
						ColumnDef::new(Sources::LastSeenAt)
							.timestamp_with_time_zone()
							.not_null(),
					)
					.to_owned(),
			)
			.await?;

		// Attachment resolution walks every source on a mount event, and path
		// resolution is longest-prefix over roots.
		manager
			.create_index(
				Index::create()
					.name("idx_sources_volume_uuid")
					.table(Sources::Table)
					.col(Sources::VolumeUuid)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_sources_data_type")
					.table(Sources::Table)
					.col(Sources::DataType)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_table(Table::drop().table(Sources::Table).to_owned())
			.await
	}
}

#[derive(DeriveIden)]
enum Sources {
	Table,
	Id,
	Uuid,
	Name,
	DataType,
	AdapterId,
	Config,
	Root,
	VolumeUuid,
	RecordCount,
	DirectoryCount,
	TotalBytes,
	UniqueBytes,
	LastIndexedAt,
	Status,
	TrustTier,
	CreatedAt,
	LastSeenAt,
}
