//! The image facet: what an EXIF read says about a file's bytes.
//!
//! `facet_image` is keyed by record like every facet, but what it describes
//! is the bytes, so the row also carries the hash of the content it was read
//! from. That hash is the re-run guard: a record whose content hash no
//! longer matches its facet row is pending again, and one whose row matches
//! is never read twice. A walk that sees a file change clears the record's
//! content id, identification assigns a new one, and the mismatch queues the
//! file here without the two phases ever coordinating.
//!
//! Every record sharing the bytes takes the same row in one statement, so a
//! source holding a photo three times reads its header once.

use uuid::Uuid;

use crate::error::Result;
use crate::file::address;

/// The hash a facet row is keyed by. The sampled hash survives verification
/// (a confirmed row keeps it), so a verified file does not read as changed.
pub(crate) const CONTENT_KEY: &str = "COALESCE(c.sampled_hash, c.integrity_hash)";

/// [`CONTENT_KEY`] as a lookup, with the hash bound twice. Spelled out so
/// each arm reaches its own index; a `COALESCE` on the left of `=` is a
/// scan of `content` per statement.
pub(crate) const CONTENT_KEY_IS: &str =
	"(c.sampled_hash = ? OR (c.sampled_hash IS NULL AND c.integrity_hash = ?))";

/// Records of one content kind whose row in a content-keyed facet table is
/// missing or describes other bytes.
///
/// One clause, so the count and the batch cannot disagree about what is
/// outstanding. A file whose bytes could not be read has no content row and
/// is not here; it waits for identification like everything else.
fn pending_clause(table: &str) -> String {
	format!(
		"FROM record r \
		 JOIN content c ON c.id = r.content_id \
		 JOIN facet_file f ON f.record_uuid = r.uuid \
		 LEFT JOIN directory_path d ON d.record_uuid = r.parent_uuid \
		 LEFT JOIN {table} i ON i.record_uuid = r.uuid \
		 WHERE r.type = 'file' AND c.kind = ? \
		 AND (i.record_uuid IS NULL OR i.content_hash IS NOT {CONTENT_KEY})"
	)
}

/// What an EXIF read yields, in the facet's own columns. Every field is
/// optional: a row with nothing but a content hash records that the bytes
/// were read and carried no EXIF, which is what keeps them out of the
/// pending set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageFacet {
	pub width: Option<i64>,
	pub height: Option<i64>,
	/// RFC 3339.
	pub date_taken: Option<String>,
	pub latitude: Option<f64>,
	pub longitude: Option<f64>,
	pub camera_make: Option<String>,
	pub camera_model: Option<String>,
	pub lens_model: Option<String>,
	pub focal_length: Option<String>,
	pub aperture: Option<String>,
	pub shutter_speed: Option<String>,
	pub iso: Option<i64>,
	/// The EXIF orientation value, 1 through 8.
	pub orientation: Option<i64>,
	pub color_space: Option<String>,
	pub color_profile: Option<String>,
	pub bit_depth: Option<String>,
	pub artist: Option<String>,
	pub copyright: Option<String>,
	pub description: Option<String>,
}

/// A record waiting for a facet row read from its bytes.
#[derive(Debug, Clone)]
pub struct PendingFacet {
	pub uuid: Uuid,
	/// Row id, the cursor a pass claims by.
	pub rowid: i64,
	/// Path relative to the source root.
	pub external_id: String,
	/// The hash the facet row will be keyed by.
	pub content_hash: String,
}

type PendingFacetRow = (
	Uuid,
	i64,
	Option<Uuid>,
	Option<String>,
	Option<String>,
	String,
);

/// How many image records are still waiting. `kind` is the content kind
/// discriminant the caller stores for images.
pub async fn count_files_needing_image_facets(pool: &sqlx::SqlitePool, kind: i64) -> Result<i64> {
	count_pending(pool, "facet_image", kind).await
}

pub(crate) async fn count_pending(pool: &sqlx::SqlitePool, table: &str, kind: i64) -> Result<i64> {
	Ok(
		sqlx::query_scalar(&format!("SELECT COUNT(*) {}", pending_clause(table)))
			.bind(kind)
			.fetch_one(pool)
			.await?,
	)
}

/// Image records past `after` with no current facet row, in row order.
///
/// Claimed by cursor rather than from the start each time, so a file the
/// pass could not read is passed over rather than handed back forever.
pub async fn files_needing_image_facets(
	pool: &sqlx::SqlitePool,
	kind: i64,
	after: i64,
	batch_size: usize,
) -> Result<Vec<PendingFacet>> {
	pending(pool, "facet_image", kind, after, batch_size).await
}

pub(crate) async fn pending(
	pool: &sqlx::SqlitePool,
	table: &str,
	kind: i64,
	after: i64,
	batch_size: usize,
) -> Result<Vec<PendingFacet>> {
	let rows: Vec<PendingFacetRow> = sqlx::query_as(&format!(
		"SELECT r.uuid, r.rowid, r.parent_uuid, d.path, r.title, {CONTENT_KEY} \
		 {} AND r.rowid > ? ORDER BY r.rowid LIMIT ?",
		pending_clause(table)
	))
	.bind(kind)
	.bind(after)
	.bind(batch_size as i64)
	.fetch_all(pool)
	.await?;

	Ok(rows
		.into_iter()
		.filter_map(
			|(uuid, rowid, parent_uuid, parent_path, title, content_hash)| {
				Some(PendingFacet {
					uuid,
					rowid,
					external_id: address(parent_uuid, parent_path, title)?,
					content_hash,
				})
			},
		)
		.collect())
}

const IMAGE_COLUMNS: &str = "width, height, date_taken, latitude, longitude, camera_make, \
	camera_model, lens_model, focal_length, aperture, shutter_speed, iso, orientation, \
	color_space, color_profile, bit_depth, artist, copyright, description";

/// Write a facet row to every record holding the hashed bytes.
///
/// One statement per content hash reaches every copy, so the row is read
/// once however many times a source holds the photo. A record that gains
/// the content later is pending again and reads the header itself; that is
/// one small read, not a correctness problem.
pub async fn set_image_facets(
	pool: &sqlx::SqlitePool,
	facets: &[(String, ImageFacet)],
) -> Result<u64> {
	let updates = IMAGE_COLUMNS
		.split(", ")
		.map(|column| format!("{column} = excluded.{column}"))
		.collect::<Vec<_>>()
		.join(", ");
	let sql = format!(
		"INSERT INTO facet_image (record_uuid, content_hash, {IMAGE_COLUMNS}) \
		 SELECT r.uuid, ?, {} FROM record r JOIN content c ON c.id = r.content_id \
		 WHERE {CONTENT_KEY_IS} \
		 ON CONFLICT (record_uuid) DO UPDATE SET content_hash = excluded.content_hash, {updates}",
		vec!["?"; 19].join(", ")
	);

	let mut tx = pool.begin().await?;
	let mut written = 0;
	for (hash, facet) in facets {
		written += sqlx::query(&sql)
			.bind(hash)
			.bind(facet.width)
			.bind(facet.height)
			.bind(&facet.date_taken)
			.bind(facet.latitude)
			.bind(facet.longitude)
			.bind(&facet.camera_make)
			.bind(&facet.camera_model)
			.bind(&facet.lens_model)
			.bind(&facet.focal_length)
			.bind(&facet.aperture)
			.bind(&facet.shutter_speed)
			.bind(facet.iso)
			.bind(facet.orientation)
			.bind(&facet.color_space)
			.bind(&facet.color_profile)
			.bind(&facet.bit_depth)
			.bind(&facet.artist)
			.bind(&facet.copyright)
			.bind(&facet.description)
			.bind(hash)
			.bind(hash)
			.execute(&mut *tx)
			.await?
			.rows_affected();
	}
	tx.commit().await?;
	Ok(written)
}
