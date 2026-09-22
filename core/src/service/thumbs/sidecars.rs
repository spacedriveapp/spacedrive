//! # Thumbnail sidecars
//!
//! The durable tier under the hot cache. Every tile baked here is also kept as
//! WebP in its source's `sidecars.db`, 5 to 15 KB against the 300 KB or so the
//! cache holds it in, so the cache can be refilled without reading the
//! original file again, and a peer can keep a copy of every tile a source has.
//!
//! Sidecars are keyed by record uuid rather than by content hash. Every file
//! has a uuid from its first walk, while a content id arrives later and is
//! re-derived when a full hash lands, which would strand anything keyed by it.
//! A duplicate costs one tile per copy, which at this size is cheap.
//!
//! A store counts its writes, and each tile carries the count it was written
//! at, so a replica that knows how far it has read asks for everything after
//! that. The count is paired with an id drawn when the file is created, so a
//! recreated store is noticed rather than read from the middle. A replica
//! keeps its copy of a peer's store in the same format, recording the owner's
//! id and how far it has read in place of its own.
//!
//! WebP bytes live in a table of their own, so the index of which tiles a
//! store holds loads without reading a byte of image data.

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use sd_pvcache::Frame;
use sqlx::sqlite::{
	SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

use super::service::TilePixels;
use super::TILE;

const SCHEMA: [&str; 4] = [
	"CREATE TABLE IF NOT EXISTS mark (
		id INTEGER PRIMARY KEY CHECK (id = 1),
		store BLOB NOT NULL,
		cursor INTEGER NOT NULL
	)",
	"CREATE TABLE IF NOT EXISTS tile (
		uuid BLOB PRIMARY KEY,
		version INTEGER NOT NULL,
		content_width INTEGER NOT NULL,
		content_height INTEGER NOT NULL,
		source_width INTEGER NOT NULL,
		source_height INTEGER NOT NULL,
		seq INTEGER NOT NULL
	)",
	"CREATE INDEX IF NOT EXISTS tile_seq ON tile (seq)",
	"CREATE TABLE IF NOT EXISTS webp (
		uuid BLOB PRIMARY KEY,
		bytes BLOB NOT NULL
	)",
];

const WEBP_QUALITY: f32 = 80.0;

/// A tile as a sidecar holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTile {
	pub version: u64,
	pub frame: Frame,
	pub webp: Vec<u8>,
}

/// A tile and the position it was written at, as a replica reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct SidecarRow {
	pub seq: u64,
	pub uuid: Uuid,
	pub tile: StoredTile,
}

/// One `sidecars.db`, this device's for a source or its copy of a peer's.
pub struct SidecarStore {
	pool: SqlitePool,
	/// The version of every tile held, so asking whether one is here costs
	/// no query.
	versions: Mutex<HashMap<Uuid, u64>>,
}

impl SidecarStore {
	/// Open a sidecar file, creating it and its directory when missing.
	pub async fn open(path: &Path) -> anyhow::Result<Self> {
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}
		// Every tile here can be made again from the hot cache or its file, so
		// a commit need not wait for the disk: in WAL mode that risks only the
		// last few commits on power loss, never the file.
		let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))?
			.create_if_missing(true)
			.journal_mode(SqliteJournalMode::Wal)
			.synchronous(SqliteSynchronous::Normal)
			.busy_timeout(Duration::from_secs(5));
		let pool = SqlitePoolOptions::new()
			.max_connections(4)
			.connect_with(options)
			.await?;
		for statement in SCHEMA {
			sqlx::query(statement).execute(&pool).await?;
		}
		sqlx::query("INSERT OR IGNORE INTO mark (id, store, cursor) VALUES (1, ?, 0)")
			.bind(Uuid::now_v7().as_bytes().as_slice())
			.execute(&pool)
			.await?;

		let versions = sqlx::query_as::<_, (Vec<u8>, i64)>("SELECT uuid, version FROM tile")
			.fetch_all(&pool)
			.await?
			.into_iter()
			.filter_map(|(uuid, version)| Some((Uuid::from_slice(&uuid).ok()?, version as u64)))
			.collect();
		Ok(Self {
			pool,
			versions: Mutex::new(versions),
		})
	}

	/// The version of `uuid`'s tile held here, if any.
	pub fn version_of(&self, uuid: Uuid) -> Option<u64> {
		self.versions
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.get(&uuid)
			.copied()
	}

	/// How many tiles this store holds.
	pub fn len(&self) -> usize {
		self.versions
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub async fn read(&self, uuid: Uuid) -> anyhow::Result<Option<StoredTile>> {
		let row = sqlx::query(
			"SELECT tile.version, tile.content_width, tile.content_height,
				tile.source_width, tile.source_height, webp.bytes
			FROM tile JOIN webp ON webp.uuid = tile.uuid
			WHERE tile.uuid = ?",
		)
		.bind(uuid.as_bytes().as_slice())
		.fetch_optional(&self.pool)
		.await?;
		Ok(row.map(|row| stored_tile(&row)))
	}

	/// This store's id and how far it has written. For a replica's copy, the
	/// owner's id and how far the copy has read.
	pub async fn mark(&self) -> anyhow::Result<(Uuid, u64)> {
		let (store, cursor): (Vec<u8>, i64) =
			sqlx::query_as("SELECT store, cursor FROM mark WHERE id = 1")
				.fetch_one(&self.pool)
				.await?;
		Ok((Uuid::from_slice(&store)?, cursor as u64))
	}

	/// Keep tiles this device baked, each at the next position.
	pub async fn record(&self, tiles: Vec<(Uuid, StoredTile)>) -> anyhow::Result<()> {
		if tiles.is_empty() {
			return Ok(());
		}
		let mut tx = self.pool.begin().await?;
		// Reserving the positions is the transaction's first statement, so it
		// takes the write lock before reading the cursor it advances.
		let last: i64 =
			sqlx::query_scalar("UPDATE mark SET cursor = cursor + ? WHERE id = 1 RETURNING cursor")
				.bind(tiles.len() as i64)
				.fetch_one(&mut *tx)
				.await?;
		let first = last as u64 - tiles.len() as u64 + 1;
		for (offset, (uuid, tile)) in tiles.iter().enumerate() {
			put(&mut tx, *uuid, tile, first + offset as u64).await?;
		}
		tx.commit().await?;
		self.remember(tiles.iter().map(|(uuid, tile)| (*uuid, tile.version)));
		Ok(())
	}

	/// Keep tiles fetched from their owner outside its sequence. The cursor
	/// stays where it is, since these say nothing about how far the copy has
	/// read.
	pub async fn keep(&self, tiles: Vec<(Uuid, StoredTile)>) -> anyhow::Result<()> {
		if tiles.is_empty() {
			return Ok(());
		}
		let mut tx = self.pool.begin().await?;
		for (uuid, tile) in &tiles {
			put(&mut tx, *uuid, tile, 0).await?;
		}
		tx.commit().await?;
		self.remember(tiles.iter().map(|(uuid, tile)| (*uuid, tile.version)));
		Ok(())
	}

	/// Tiles written after `seq`, in the order they were written, at most
	/// `limit` of them.
	pub async fn rows_after(&self, seq: u64, limit: u32) -> anyhow::Result<Vec<SidecarRow>> {
		let rows = sqlx::query(
			"SELECT tile.seq, tile.uuid, tile.version, tile.content_width,
				tile.content_height, tile.source_width, tile.source_height, webp.bytes
			FROM tile JOIN webp ON webp.uuid = tile.uuid
			WHERE tile.seq > ? ORDER BY tile.seq LIMIT ?",
		)
		.bind(seq as i64)
		.bind(i64::from(limit))
		.fetch_all(&self.pool)
		.await?;
		rows.iter()
			.map(|row| {
				Ok(SidecarRow {
					seq: row.try_get::<i64, _>("seq")? as u64,
					uuid: Uuid::from_slice(&row.try_get::<Vec<u8>, _>("uuid")?)?,
					tile: stored_tile(row),
				})
			})
			.collect()
	}

	/// Apply rows read from the owner store `store`, and move the cursor to
	/// the last of them. Rows from a store other than the one this copy has
	/// been reading replace its place in the old one.
	pub async fn apply(&self, store: Uuid, rows: Vec<SidecarRow>) -> anyhow::Result<()> {
		let (current, cursor) = self.mark().await?;
		let reached = rows.iter().map(|row| row.seq).max();
		let cursor = match reached {
			Some(reached) if current == store => reached.max(cursor),
			Some(reached) => reached,
			None if current == store => cursor,
			None => 0,
		};

		let mut tx = self.pool.begin().await?;
		for row in &rows {
			put(&mut tx, row.uuid, &row.tile, row.seq).await?;
		}
		sqlx::query("UPDATE mark SET store = ?, cursor = ? WHERE id = 1")
			.bind(store.as_bytes().as_slice())
			.bind(cursor as i64)
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		self.remember(rows.iter().map(|row| (row.uuid, row.tile.version)));
		Ok(())
	}

	fn remember(&self, tiles: impl Iterator<Item = (Uuid, u64)>) {
		self.versions
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.extend(tiles);
	}
}

async fn put(
	tx: &mut Transaction<'_, Sqlite>,
	uuid: Uuid,
	tile: &StoredTile,
	seq: u64,
) -> anyhow::Result<()> {
	sqlx::query(
		"INSERT OR REPLACE INTO tile (uuid, version, content_width, content_height,
			source_width, source_height, seq) VALUES (?, ?, ?, ?, ?, ?, ?)",
	)
	.bind(uuid.as_bytes().as_slice())
	.bind(tile.version as i64)
	.bind(i64::from(tile.frame.content_width))
	.bind(i64::from(tile.frame.content_height))
	.bind(i64::from(tile.frame.source_width))
	.bind(i64::from(tile.frame.source_height))
	.bind(seq as i64)
	.execute(&mut **tx)
	.await?;
	sqlx::query("INSERT OR REPLACE INTO webp (uuid, bytes) VALUES (?, ?)")
		.bind(uuid.as_bytes().as_slice())
		.bind(tile.webp.as_slice())
		.execute(&mut **tx)
		.await?;
	Ok(())
}

fn stored_tile(row: &sqlx::sqlite::SqliteRow) -> StoredTile {
	let dimension = |column: &str| row.get::<i64, _>(column) as u32;
	StoredTile {
		version: row.get::<i64, _>("version") as u64,
		frame: Frame {
			content_width: dimension("content_width"),
			content_height: dimension("content_height"),
			source_width: dimension("source_width"),
			source_height: dimension("source_height"),
		},
		webp: row.get("bytes"),
	}
}

/// A tile as lossy WebP. Lossy is fine at this size: a tile is already a
/// downscale, shown at or below its own resolution.
pub(super) fn encode(tile: &TilePixels) -> Option<Vec<u8>> {
	let Frame {
		content_width: width,
		content_height: height,
		..
	} = tile.frame;
	if width == 0 || height == 0 || tile.bgra.len() != tile.frame.len() {
		return None;
	}
	// Tiles are stored BGRA8, which is what a GPU atlas wants; WebP takes RGBA.
	let mut rgba = tile.bgra.clone();
	for pixel in rgba.chunks_exact_mut(4) {
		pixel.swap(0, 2);
	}
	Some(
		webp::Encoder::from_rgba(&rgba, width, height)
			.encode(WEBP_QUALITY)
			.to_vec(),
	)
}

/// A WebP tile back to BGRA8, provided it has the size `frame` declares and
/// fits a slot.
pub(super) fn decode(bytes: &[u8], frame: Frame) -> Option<Vec<u8>> {
	if frame.content_width > TILE || frame.content_height > TILE {
		return None;
	}
	let image = webp::Decoder::new(bytes).decode()?;
	if image.width() != frame.content_width || image.height() != frame.content_height {
		return None;
	}
	// The decoder drops the alpha channel of an opaque image.
	let channels = if image.is_alpha() { 4 } else { 3 };
	let mut bgra = Vec::with_capacity(frame.len());
	for pixel in image.chunks_exact(channels) {
		let alpha = if channels == 4 { pixel[3] } else { u8::MAX };
		bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], alpha]);
	}
	(bgra.len() == frame.len()).then_some(bgra)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn frame(width: u32, height: u32) -> Frame {
		Frame {
			content_width: width,
			content_height: height,
			source_width: width * 10,
			source_height: height * 10,
		}
	}

	/// A gradient, so a lossy round trip has something to get wrong.
	fn gradient(width: u32, height: u32, alpha: impl Fn(u32, u32) -> u8) -> TilePixels {
		let mut bgra = Vec::new();
		for y in 0..height {
			for x in 0..width {
				bgra.extend_from_slice(&[
					(x * 255 / width) as u8,
					(y * 255 / height) as u8,
					128,
					alpha(x, y),
				]);
			}
		}
		TilePixels {
			version: 7,
			frame: frame(width, height),
			bgra,
		}
	}

	fn max_error(a: &[u8], b: &[u8]) -> u8 {
		a.iter()
			.zip(b)
			.map(|(a, b)| a.abs_diff(*b))
			.max()
			.unwrap_or(0)
	}

	fn stored(version: u64, byte: u8) -> StoredTile {
		StoredTile {
			version,
			frame: frame(2, 2),
			webp: vec![byte; 3],
		}
	}

	async fn store(dir: &tempfile::TempDir, name: &str) -> SidecarStore {
		SidecarStore::open(&dir.path().join(name).join("sidecars.db"))
			.await
			.expect("opens")
	}

	#[test]
	fn an_opaque_tile_survives_encoding() {
		let tile = gradient(384, 216, |_, _| 255);
		let bytes = encode(&tile).expect("encodes");
		assert!(
			bytes.len() < tile.bgra.len() / 10,
			"{} bytes for {} of pixels",
			bytes.len(),
			tile.bgra.len()
		);

		let decoded = decode(&bytes, tile.frame).expect("decodes");
		assert_eq!(decoded.len(), tile.bgra.len());
		assert!(decoded.chunks_exact(4).all(|pixel| pixel[3] == 255));
		assert!(
			max_error(&decoded, &tile.bgra) < 24,
			"channels stay in order"
		);
	}

	#[test]
	fn transparency_survives_encoding() {
		let tile = gradient(64, 64, |x, _| if x < 32 { 0 } else { 255 });
		let decoded = decode(&encode(&tile).expect("encodes"), tile.frame).expect("decodes");
		assert_eq!(decoded[3], 0, "the left edge stays transparent");
		assert_eq!(
			decoded[decoded.len() - 1],
			255,
			"the right edge stays opaque"
		);
	}

	#[test]
	fn a_tile_that_disagrees_with_its_frame_is_refused() {
		let tile = gradient(64, 48, |_, _| 255);
		let bytes = encode(&tile).expect("encodes");
		assert!(decode(&bytes, frame(48, 64)).is_none(), "wrong size");
		assert!(
			decode(&bytes, frame(TILE + 1, 48)).is_none(),
			"wider than a slot"
		);
		assert!(decode(b"not a webp", tile.frame).is_none(), "not an image");
	}

	#[tokio::test]
	async fn each_write_takes_the_next_position_and_a_rewrite_moves_the_tile() {
		let dir = tempfile::tempdir().unwrap();
		let owner = store(&dir, "owner").await;
		let (a, b) = (Uuid::now_v7(), Uuid::now_v7());

		owner
			.record(vec![(a, stored(1, 1)), (b, stored(1, 2))])
			.await
			.unwrap();
		let (id, cursor) = owner.mark().await.unwrap();
		assert_eq!(cursor, 2);
		assert_eq!(owner.version_of(a), Some(1));

		owner.record(vec![(a, stored(2, 3))]).await.unwrap();
		let rows = owner.rows_after(0, 10).await.unwrap();
		assert_eq!(
			rows.iter()
				.map(|row| (row.uuid, row.seq))
				.collect::<Vec<_>>(),
			[(b, 2), (a, 3)],
			"a rewritten tile moves to the end"
		);
		assert_eq!(rows[1].tile, stored(2, 3));
		assert_eq!(owner.rows_after(2, 10).await.unwrap().len(), 1);
		assert_eq!(owner.rows_after(0, 1).await.unwrap().len(), 1, "limited");
		assert_eq!(owner.mark().await.unwrap(), (id, 3));
		assert_eq!(owner.read(a).await.unwrap(), Some(stored(2, 3)));
		assert_eq!(owner.read(Uuid::now_v7()).await.unwrap(), None);
	}

	#[tokio::test]
	async fn a_copy_follows_its_owner_and_starts_over_when_the_owner_is_recreated() {
		let dir = tempfile::tempdir().unwrap();
		let owner = store(&dir, "owner").await;
		let copy = store(&dir, "copy").await;
		let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
		owner
			.record(vec![(a, stored(1, 1)), (b, stored(1, 2))])
			.await
			.unwrap();
		let (owner_id, _) = owner.mark().await.unwrap();

		copy.apply(owner_id, owner.rows_after(0, 1).await.unwrap())
			.await
			.unwrap();
		assert_eq!(copy.mark().await.unwrap(), (owner_id, 1));
		copy.apply(owner_id, owner.rows_after(1, 10).await.unwrap())
			.await
			.unwrap();
		assert_eq!(copy.mark().await.unwrap(), (owner_id, 2));
		assert_eq!(copy.read(b).await.unwrap(), Some(stored(1, 2)));

		let fetched = Uuid::now_v7();
		copy.keep(vec![(fetched, stored(9, 9))]).await.unwrap();
		assert_eq!(copy.version_of(fetched), Some(9));
		assert_eq!(
			copy.mark().await.unwrap(),
			(owner_id, 2),
			"a fetched tile says nothing about how far the copy has read"
		);

		let recreated = Uuid::now_v7();
		copy.apply(recreated, Vec::new()).await.unwrap();
		assert_eq!(copy.mark().await.unwrap(), (recreated, 0));
		assert_eq!(copy.version_of(a), Some(1), "tiles already held stay");
	}

	#[tokio::test]
	async fn a_store_reopens_with_its_mark_and_index() {
		let dir = tempfile::tempdir().unwrap();
		let uuid = Uuid::now_v7();
		let mark = {
			let owner = store(&dir, "owner").await;
			owner.record(vec![(uuid, stored(4, 4))]).await.unwrap();
			owner.mark().await.unwrap()
		};
		let reopened = store(&dir, "owner").await;
		assert_eq!(reopened.mark().await.unwrap(), mark);
		assert_eq!(reopened.version_of(uuid), Some(4));
		assert_eq!(reopened.len(), 1);

		let other = store(&dir, "other").await;
		assert_ne!(
			other.mark().await.unwrap().0,
			mark.0,
			"each store draws its own id"
		);
	}
}
