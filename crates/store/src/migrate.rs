//! # Store schema version
//!
//! A source store carries its schema version in SQLite's `PRAGMA
//! user_version`. The number names the shape of the generation tables, not
//! the data type's facet columns, which [`crate::db::SourceDb::ensure_facet_columns`]
//! adds on their own.
//!
//! Only a writer migrates. It runs every migration between the store's version
//! and [`SCHEMA_VERSION`], in order, each in its own transaction that also
//! writes the new version, before the record DDL and before the revision
//! triggers are installed. A fresh store skips straight to the current version
//! because the DDL that follows creates the current shape.
//!
//! A replica is a `VACUUM INTO` copy of the owner's store and so carries the
//! owner's version. Readers of a replica cannot migrate it and must read
//! whichever shape its version names until the owner upgrades.

use sqlx::SqlitePool;

use crate::error::{Error, Result};

/// The version a writer brings a store to.
pub const SCHEMA_VERSION: i64 = 0;

/// The version a store carries. `0` for a store written before versions
/// existed.
pub async fn version(pool: &SqlitePool) -> Result<i64> {
	Ok(sqlx::query_scalar("PRAGMA user_version")
		.fetch_one(pool)
		.await?)
}

/// Bring a store to [`SCHEMA_VERSION`].
///
/// Refuses a store written by a newer Spacedrive, because downgrading would
/// read rows through a shape they do not have.
pub(crate) async fn run(pool: &SqlitePool) -> Result<()> {
	let current = version(pool).await?;
	if current > SCHEMA_VERSION {
		return Err(Error::UnsupportedGeneration(format!(
			"store schema version {current} is newer than this build's {SCHEMA_VERSION}"
		)));
	}
	if current == SCHEMA_VERSION {
		return Ok(());
	}

	let has_record: Option<i64> =
		sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'record'")
			.fetch_optional(pool)
			.await?;
	if has_record.is_none() {
		sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
			.execute(pool)
			.await?;
		return Ok(());
	}

	for next in (current + 1)..=SCHEMA_VERSION {
		apply(pool, next).await?;
		tracing::info!(version = next, "migrated source store");
	}

	Ok(())
}

/// One migration, from `next - 1` to `next`, committed with its version.
async fn apply(_pool: &SqlitePool, next: i64) -> Result<()> {
	Err(Error::Other(format!(
		"no migration defined for schema version {next}"
	)))
}
