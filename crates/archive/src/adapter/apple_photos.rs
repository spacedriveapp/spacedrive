//! Apple Photos native adapter.
//!
//! Harvests a `.photoslibrary` bundle by snapshotting its `Photos.sqlite`
//! (Core Data) store and projecting assets, user albums, and named people onto
//! the record table. All Apple-specific knowledge lives here: the Core Data
//! projection, the derivative fallback chain, and the album/person join
//! resolution.
//!
//! The harvest is strictly read-only. The live database is copied before any
//! query runs, originals and derivatives are referenced by absolute path
//! rather than materialized, and nothing is written back to the library.
//! Source mutations (e.g. pushing favorite/hidden state into Photos) would
//! attach here as separate connector-owned actions, never on the sync path.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Instant;

use rusqlite::Connection;
use serde_json::json;

use crate::adapter::script::ConfigField;
use crate::adapter::{Adapter, AdapterKind, SyncReport};
use crate::db::SourceDb;
use crate::error::{Error, Result};
use crate::registry::TrustTier;
use crate::schema::DataTypeSchema;

/// Model names, shared between the schema declaration and the ingest path.
const PHOTO: &str = "photo";
const ALBUM: &str = "album";
const PERSON: &str = "person";

/// The data type schema, declared in the same TOML dialect script adapters
/// use so both kinds flow through one parser and DDL generator.
const SCHEMA_TOML: &str = r#"
[data_type]
id = "photo"
name = "Photo"
icon = "image"

[models.photo]
fields.filename = "string"
fields.captured_at = "datetime"
fields.modified_at = "datetime"
fields.media_type = "string"
fields.width = "integer"
fields.height = "integer"
fields.burst_id = "string"
fields.available = "boolean"
fields.favorite = "boolean"
fields.hidden = "boolean"
fields.camera_make = "string"
fields.camera_model = "string"
fields.lens = "string"
fields.iso = "integer"
fields.aperture = "float"
fields.shutter = "float"
fields.focal_length = "float"
fields.focal_length_35 = "integer"
fields.exposure_bias = "float"
fields.flash = "boolean"
fields.gps_lat = "float"
fields.gps_lon = "float"
fields.original_path = "path"
fields.thumb_path = "string"
fields.thumb_px = "integer"

[models.album]
fields.name = "string"

[models.person]
fields.name = "string"

[search]
primary_model = "photo"
title = "filename"
preview = "original_path"
subtitle = "camera_model"
search_fields = ["filename", "camera_make", "camera_model", "lens"]
date_field = "captured_at"
"#;

/// Best-thumbnail fallback chain: (filename suffix, pixel tier).
const THUMB_CHAIN: &[(&str, i64)] = &[
	("_1_105_c.jpeg", 1180),
	("_1_102_o.jpeg", 576),
	("_1_100_o.jpeg", 576),
	(".THM", 32),
];

/// The compiled-in Apple Photos adapter. One instance serves every source;
/// the library to harvest comes from each source's config.
pub struct ApplePhotosAdapter {
	schema: DataTypeSchema,
}

impl ApplePhotosAdapter {
	pub fn new() -> Self {
		let schema = crate::schema::parser::parse(SCHEMA_TOML)
			.expect("apple-photos schema is declared in-crate and must parse");
		Self { schema }
	}
}

impl Default for ApplePhotosAdapter {
	fn default() -> Self {
		Self::new()
	}
}

impl Adapter for ApplePhotosAdapter {
	fn id(&self) -> &str {
		"apple-photos"
	}

	fn name(&self) -> &str {
		"Apple Photos"
	}

	fn kind(&self) -> AdapterKind {
		AdapterKind::Native
	}

	fn schema(&self) -> &DataTypeSchema {
		&self.schema
	}

	fn config_fields(&self) -> Vec<ConfigField> {
		vec![ConfigField {
			key: "library_path".to_string(),
			name: "Library path".to_string(),
			description: "Path to the .photoslibrary bundle. Defaults to the system photo library."
				.to_string(),
			field_type: "path".to_string(),
			required: false,
			secret: false,
			default: None,
			options: Vec::new(),
			path_type: Some("directory".to_string()),
		}]
	}

	fn description(&self) -> &str {
		"Index photos, videos, albums, and people from an Apple Photos library (macOS). \
		 Read-only: the library database is snapshotted before querying."
	}

	fn version(&self) -> &str {
		"0.1.0"
	}

	fn author(&self) -> &str {
		"spacedrive"
	}

	fn trust_tier(&self) -> TrustTier {
		TrustTier::Authored
	}

	fn sync<'a>(
		&'a self,
		db: &'a SourceDb,
		config: &'a serde_json::Value,
	) -> Pin<Box<dyn Future<Output = Result<SyncReport>> + Send + 'a>> {
		Box::pin(async move {
			let start = Instant::now();
			let library = library_path(config)?;
			let snapshot = snapshot_dir(config);

			// The library is read synchronously (rusqlite + directory walks) off
			// the async executor; only finished, owned data crosses back for the
			// record writes.
			let harvest_library = library.clone();
			let (assets, groups) =
				tokio::task::spawn_blocking(move || harvest(&harvest_library, &snapshot))
					.await
					.map_err(|e| Error::AdapterSync(format!("harvest task failed: {e}")))??;

			let mut report = SyncReport {
				records_upserted: 0,
				records_deleted: 0,
				links_created: 0,
				links_removed: 0,
				duration_ms: 0,
				error: None,
			};

			// File-backed contract: photo rows are assertions about files the
			// filesystem index owns, carrying library-relative path evidence.
			// The resolved root travels in _sync_state so readers can join
			// the evidence to filesystem records without knowing the config.
			db.set_cursor(crate::db::FILE_ROOT_CURSOR, &library.to_string_lossy())
				.await?;

			// Map Apple's per-asset primary key to our external id, so album and
			// person membership (expressed in Z_PK terms by Core Data) can be
			// turned into edges between records.
			let mut zpk_to_ext: HashMap<i64, &str> = HashMap::with_capacity(assets.len());
			for asset in &assets {
				db.upsert(PHOTO, &asset.zuuid, &photo_fields(asset)).await?;
				zpk_to_ext.insert(asset.z_pk, asset.zuuid.as_str());
				report.records_upserted += 1;
			}

			// Membership edges carry no per-row change token, so they are rebuilt
			// wholesale: clearing first means a removed album's edges vanish
			// without a per-edge diff.
			sqlx::query("DELETE FROM edge WHERE type IN (?, ?)")
				.bind(ALBUM)
				.bind(PERSON)
				.execute(db.pool())
				.await?;

			for group in &groups {
				db.upsert(
					group.model,
					&group.external_id,
					&json!({ "name": group.title }),
				)
				.await?;
				report.records_upserted += 1;

				for zpk in &group.member_zpks {
					if let Some(ext) = zpk_to_ext.get(zpk) {
						db.link(PHOTO, ext, group.model, &group.external_id).await?;
						report.links_created += 1;
					}
				}
			}

			report.duration_ms = start.elapsed().as_millis() as u64;
			Ok(report)
		})
	}
}

/// The library to harvest: the configured path, or the system default.
fn library_path(config: &serde_json::Value) -> Result<PathBuf> {
	if let Some(path) = config
		.get("library_path")
		.and_then(|v| v.as_str())
		.filter(|s| !s.is_empty())
	{
		return Ok(PathBuf::from(path));
	}

	dirs::home_dir()
		.map(|home| home.join("Pictures/Photos Library.photoslibrary"))
		.ok_or_else(|| {
			Error::AdapterSync(
				"no library_path configured and no home directory to derive the default from"
					.to_string(),
			)
		})
}

/// Where the database snapshot lives: inside the source's data directory, so
/// it is cleaned up with the source.
fn snapshot_dir(config: &serde_json::Value) -> PathBuf {
	match config.get("_data_dir").and_then(|v| v.as_str()) {
		Some(dir) => PathBuf::from(dir).join("snapshot"),
		None => std::env::temp_dir().join("spacedrive-apple-photos-snapshot"),
	}
}

/// Read everything the sync needs from the library, synchronously: snapshot
/// the database, project assets, resolve thumbnails, extract groups.
fn harvest(library: &Path, snapshot: &Path) -> Result<(Vec<Asset>, Vec<GroupSrc>)> {
	let copy = snapshot_db(&library.join("database/Photos.sqlite"), snapshot)?;
	let mut assets = read_assets(&copy)?;
	resolve_thumbs(&mut assets, &library.join("resources"));
	let groups = read_groups(&copy)?;
	Ok((assets, groups))
}

/// One asset projected from Apple's Core Data, plus resolved thumbnail info.
struct Asset {
	/// Core Data primary key — joins albums/faces to this asset.
	z_pk: i64,
	zuuid: String,
	original_filename: Option<String>,
	/// Library-relative original location: `originals/<ZDIRECTORY>/<ZFILENAME>`.
	original_path: String,
	/// Core Data timestamp: seconds since 2001-01-01 UTC.
	date_created: f64,
	mod_date: Option<f64>,
	/// ZKIND: 0 = image, 1 = video.
	kind: i64,
	width: Option<i64>,
	height: Option<i64>,
	favorite: bool,
	hidden: bool,
	burst_id: Option<String>,
	// EXIF, from ZEXTENDEDATTRIBUTES. Values are stored directly (aperture is
	// the f-number, shutter is exposure seconds) — no APEX conversion needed.
	camera_make: Option<String>,
	camera_model: Option<String>,
	lens: Option<String>,
	iso: Option<i64>,
	aperture: Option<f64>,
	shutter: Option<f64>,
	focal_length: Option<f64>,
	focal_length_35: Option<i64>,
	exposure_bias: Option<f64>,
	flash: Option<bool>,
	gps_lat: Option<f64>,
	gps_lon: Option<f64>,
	/// Resources-relative derivative location, once resolved.
	thumb_path: Option<String>,
	thumb_px: Option<i64>,
	available: bool,
}

/// A grouping record to materialize: an album or named person, with its
/// member assets identified by Apple's `Z_PK`.
struct GroupSrc {
	model: &'static str,
	external_id: String,
	title: String,
	member_zpks: Vec<i64>,
}

/// Copy `Photos.sqlite` (with its WAL and SHM sidecars) so queries run against
/// a stable snapshot without ever touching the live library's WAL. Recopied on
/// every sync so the run observes the library's current state.
/// Snapshot one file. On APFS a clone is instant and consumes no space
/// until the original diverges — a multi-gigabyte catalog costs nothing per
/// sync. Filesystems without cloning fall back to a byte copy.
fn snapshot_file(from: &Path, to: &Path) -> std::io::Result<()> {
	#[cfg(target_os = "macos")]
	{
		use std::os::unix::ffi::OsStrExt;
		let from_c = std::ffi::CString::new(from.as_os_str().as_bytes())
			.map_err(|e| std::io::Error::other(e.to_string()))?;
		let to_c = std::ffi::CString::new(to.as_os_str().as_bytes())
			.map_err(|e| std::io::Error::other(e.to_string()))?;
		if unsafe { libc::clonefile(from_c.as_ptr(), to_c.as_ptr(), 0) } == 0 {
			return Ok(());
		}
	}
	std::fs::copy(from, to).map(|_| ())
}

fn snapshot_db(src_db: &Path, dir: &Path) -> Result<PathBuf> {
	if !src_db.exists() {
		return Err(Error::AdapterSync(format!(
			"no Photos.sqlite at {} — is the library path correct?",
			src_db.display()
		)));
	}

	std::fs::create_dir_all(dir)?;
	let copy = dir.join("Photos.sqlite");

	// A sidecar left over from a previous snapshot must not merge into this one.
	for ext in ["", "-wal", "-shm"] {
		let to = dir.join(format!("Photos.sqlite{ext}"));
		if to.exists() {
			std::fs::remove_file(&to)?;
		}
		let from = PathBuf::from(format!("{}{ext}", src_db.display()));
		if from.exists() {
			snapshot_file(&from, &to)?;
		}
	}

	// Opening read-write folds the copied WAL into the main file and verifies
	// the snapshot is coherent before anything queries it.
	let conn = Connection::open(&copy)?;
	let verdict: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
	if verdict != "ok" {
		tracing::warn!(%verdict, "Photos.sqlite snapshot integrity check");
	}

	Ok(copy)
}

/// Project the Core Data schema into `Asset`s, ordered by capture date.
fn read_assets(copy: &Path) -> Result<Vec<Asset>> {
	let conn = Connection::open_with_flags(copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
	let mut stmt = conn.prepare(
		"SELECT a.ZUUID,
		        aa.ZORIGINALFILENAME,
		        'originals/' || a.ZDIRECTORY || '/' || a.ZFILENAME,
		        a.ZDATECREATED,
		        a.ZKIND, a.ZWIDTH, a.ZHEIGHT,
		        a.ZFAVORITE, a.ZHIDDEN, a.ZAVALANCHEUUID,
		        a.Z_PK,
		        ea.ZCAMERAMAKE, ea.ZCAMERAMODEL, ea.ZLENSMODEL,
		        ea.ZISO, ea.ZAPERTURE, ea.ZSHUTTERSPEED,
		        ea.ZFOCALLENGTH, ea.ZFOCALLENGTHIN35MM, ea.ZEXPOSUREBIAS,
		        ea.ZFLASHFIRED, ea.ZLATITUDE, ea.ZLONGITUDE,
		        a.ZMODIFICATIONDATE
		 FROM ZASSET a
		 LEFT JOIN ZADDITIONALASSETATTRIBUTES aa ON aa.ZASSET = a.Z_PK
		 LEFT JOIN ZEXTENDEDATTRIBUTES ea ON ea.ZASSET = a.Z_PK
		 ORDER BY a.ZDATECREATED ASC",
	)?;

	let rows = stmt.query_map([], |r| {
		Ok(Asset {
			zuuid: r.get(0)?,
			original_filename: r.get(1)?,
			original_path: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
			date_created: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
			mod_date: r.get::<_, Option<f64>>(23)?,
			kind: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
			width: r.get(5)?,
			height: r.get(6)?,
			favorite: r.get::<_, Option<i64>>(7)?.unwrap_or(0) != 0,
			hidden: r.get::<_, Option<i64>>(8)?.unwrap_or(0) != 0,
			burst_id: r.get(9)?,
			z_pk: r.get(10)?,
			camera_make: clean_str(r.get(11)?),
			camera_model: clean_str(r.get(12)?),
			lens: clean_str(r.get(13)?),
			iso: r.get(14)?,
			aperture: positive(r.get(15)?),
			shutter: positive(r.get(16)?),
			focal_length: positive(r.get(17)?),
			focal_length_35: r.get::<_, Option<i64>>(18)?.filter(|&v| v > 0),
			exposure_bias: r.get(19)?,
			flash: r.get::<_, Option<i64>>(20)?.map(|v| v != 0),
			gps_lat: valid_coord(r.get(21)?),
			gps_lon: valid_coord(r.get(22)?),
			thumb_path: None,
			thumb_px: None,
			available: false,
		})
	})?;

	Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Locate the album↔asset join table and its two foreign-key columns.
///
/// Core Data names many-to-many join tables after entity ordinals (e.g.
/// `Z_33ASSETS` with columns `Z_33ALBUMS` / `Z_3ASSETS`), and those ordinals
/// shift between Photos schema versions. Resolve them by shape instead: the
/// join table's name ends in `ASSETS` and it carries one column ending in
/// `ALBUMS` (the album FK) alongside one ending in `ASSETS` (the asset FK).
fn resolve_album_join(conn: &Connection) -> Option<(String, String, String)> {
	let tables: Vec<String> = {
		let mut s = conn
			.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name GLOB 'Z_*ASSETS'")
			.ok()?;
		let rows = s.query_map([], |r| r.get::<_, String>(0)).ok()?;
		rows.flatten().collect()
	};

	for table in tables {
		let cols: Vec<String> = {
			let mut s = conn
				.prepare(&format!("PRAGMA table_info('{table}')"))
				.ok()?;
			let rows = s.query_map([], |r| r.get::<_, String>(1)).ok()?;
			rows.flatten().collect()
		};
		let album_col = cols.iter().find(|c| c.ends_with("ALBUMS")).cloned();
		let asset_col = cols.iter().find(|c| c.ends_with("ASSETS")).cloned();
		if let (Some(album_col), Some(asset_col)) = (album_col, asset_col) {
			return Some((table, album_col, asset_col));
		}
	}

	None
}

/// Extract user albums (`ZGENERICALBUM` kind 2) and named people (`ZPERSON`)
/// with their asset membership. Places have no discrete grouping entity in
/// Photos (they're a map over per-asset coordinates), so none are extracted.
fn read_groups(copy: &Path) -> Result<Vec<GroupSrc>> {
	let conn = Connection::open_with_flags(copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
	let mut out = Vec::new();

	// Albums: gather membership (album Z_PK → [asset Z_PK]), then named albums.
	let mut album_members: HashMap<i64, Vec<i64>> = HashMap::new();
	match resolve_album_join(&conn) {
		Some((join_table, album_col, asset_col)) => {
			let mut stmt = conn.prepare(&format!(
				"SELECT {album_col}, {asset_col} FROM {join_table}"
			))?;
			let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
			for row in rows {
				let (album, asset) = row?;
				album_members.entry(album).or_default().push(asset);
			}
		}
		None => {
			tracing::warn!("album↔asset join table not found; skipping album membership")
		}
	}
	{
		let mut stmt = conn.prepare(
			"SELECT Z_PK, ZUUID, ZTITLE FROM ZGENERICALBUM
			 WHERE ZKIND = 2 AND ZTITLE IS NOT NULL AND ZTITLE <> '' AND ZTRASHEDSTATE = 0",
		)?;
		let rows = stmt.query_map([], |r| {
			Ok((
				r.get::<_, i64>(0)?,
				r.get::<_, String>(1)?,
				r.get::<_, String>(2)?,
			))
		})?;
		for row in rows {
			let (zpk, zuuid, title) = row?;
			let members = album_members.remove(&zpk).unwrap_or_default();
			if members.is_empty() {
				continue;
			}
			out.push(GroupSrc {
				model: ALBUM,
				external_id: format!("apple:album:{zuuid}"),
				title,
				member_zpks: members,
			});
		}
	}

	// People: gather faces (person Z_PK → [asset Z_PK]), then named people.
	let mut person_members: HashMap<i64, Vec<i64>> = HashMap::new();
	{
		let mut stmt = conn.prepare(
			"SELECT ZPERSONFORFACE, ZASSETFORFACE FROM ZDETECTEDFACE
			 WHERE ZPERSONFORFACE IS NOT NULL AND ZASSETFORFACE IS NOT NULL",
		)?;
		let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
		for row in rows {
			let (person, asset) = row?;
			person_members.entry(person).or_default().push(asset);
		}
	}
	{
		let mut stmt = conn.prepare(
			"SELECT Z_PK, ZPERSONUUID, ZFULLNAME FROM ZPERSON
			 WHERE ZFULLNAME IS NOT NULL AND ZFULLNAME <> ''",
		)?;
		let rows = stmt.query_map([], |r| {
			Ok((
				r.get::<_, i64>(0)?,
				r.get::<_, Option<String>>(1)?,
				r.get::<_, String>(2)?,
			))
		})?;
		for row in rows {
			let (zpk, puuid, name) = row?;
			let mut members = person_members.remove(&zpk).unwrap_or_default();
			// One person can have several faces in the same asset — dedupe.
			members.sort_unstable();
			members.dedup();
			if members.is_empty() {
				continue;
			}
			let ext = puuid.unwrap_or_else(|| format!("pk{zpk}"));
			out.push(GroupSrc {
				model: PERSON,
				external_id: format!("apple:person:{ext}"),
				title: name,
				member_zpks: members,
			});
		}
	}

	Ok(out)
}

/// Scan `resources/derivatives/<hexshard>/`, then resolve each asset's best
/// thumbnail by the fallback chain.
fn resolve_thumbs(assets: &mut [Asset], resources: &Path) {
	let derivatives = resources.join("derivatives");
	let present = scan_derivatives(&derivatives);

	for asset in assets.iter_mut() {
		let Some(shard) = asset.zuuid.get(..1) else {
			continue;
		};
		for (suffix, px) in THUMB_CHAIN {
			let fname = if *suffix == ".THM" {
				format!("{}.THM", asset.zuuid)
			} else {
				format!("{}{}", asset.zuuid, suffix)
			};
			let rel = format!("derivatives/{shard}/{fname}");
			if present.contains(&rel) {
				asset.thumb_path = Some(rel);
				asset.thumb_px = Some(*px);
				asset.available = true;
				break;
			}
		}
	}
}

/// Collect every derivative filename as a `derivatives/<shard>/<file>`
/// resources-relative path.
fn scan_derivatives(derivatives: &Path) -> HashSet<String> {
	let mut set = HashSet::new();
	let Ok(shards) = std::fs::read_dir(derivatives) else {
		tracing::warn!(path = %derivatives.display(), "no derivatives directory");
		return set;
	};
	for shard in shards.flatten() {
		let name = shard.file_name().to_string_lossy().to_string();
		if name.len() != 1 || !name.chars().all(|c| c.is_ascii_hexdigit()) {
			continue;
		}
		if let Ok(files) = std::fs::read_dir(shard.path()) {
			for file in files.flatten() {
				let fname = file.file_name().to_string_lossy().to_string();
				set.insert(format!("derivatives/{name}/{fname}"));
			}
		}
	}
	set
}

/// The facet fields for one asset. Locator paths are library-relative: they
/// are path evidence joining the assertion to the filesystem record that owns
/// the file, and readers absolutize them against the source's file root.
fn photo_fields(asset: &Asset) -> serde_json::Value {
	json!({
		"filename": asset.original_filename,
		"captured_at": rfc3339(to_unix_ms(asset.date_created)),
		"modified_at": asset.mod_date.and_then(|d| rfc3339(to_unix_ms(d))),
		"media_type": media_type(asset.kind),
		"width": asset.width,
		"height": asset.height,
		"burst_id": asset.burst_id,
		"available": asset.available,
		"favorite": asset.favorite,
		"hidden": asset.hidden,
		"camera_make": asset.camera_make,
		"camera_model": asset.camera_model,
		"lens": asset.lens,
		"iso": asset.iso,
		"aperture": asset.aperture,
		"shutter": asset.shutter,
		"focal_length": asset.focal_length,
		"focal_length_35": asset.focal_length_35,
		"exposure_bias": asset.exposure_bias,
		"flash": asset.flash,
		"gps_lat": asset.gps_lat,
		"gps_lon": asset.gps_lon,
		"original_path": (!asset.original_path.is_empty())
			.then(|| asset.original_path.clone()),
		"thumb_path": asset.thumb_path.as_ref().map(|rel| format!("resources/{rel}")),
		"thumb_px": asset.thumb_px,
	})
}

/// Map Apple's `ZKIND` to the facet's media type value.
fn media_type(kind: i64) -> &'static str {
	match kind {
		1 => "video",
		_ => "image",
	}
}

/// Core Data seconds-since-2001 → unix milliseconds.
fn to_unix_ms(core_data_seconds: f64) -> i64 {
	((core_data_seconds + 978_307_200.0) * 1000.0) as i64
}

/// Unix milliseconds → RFC 3339, the form datetime facet fields store.
fn rfc3339(unix_ms: i64) -> Option<String> {
	chrono::DateTime::from_timestamp_millis(unix_ms)
		.map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

/// Drop empty and placeholder strings (e.g. Sony bodies record a lens model of
/// "----" when no electronic lens is attached).
fn clean_str(s: Option<String>) -> Option<String> {
	s.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty() && !s.bytes().all(|b| b == b'-'))
}

/// Keep a numeric EXIF value only when it's positive; 0 means "not recorded".
fn positive(v: Option<f64>) -> Option<f64> {
	v.filter(|&v| v > 0.0)
}

/// Keep a coordinate only when it's a real fix; Apple stores -180 for
/// "no location".
fn valid_coord(v: Option<f64>) -> Option<f64> {
	v.filter(|&v| v > -180.0 && v <= 180.0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::source::SourceManager;

	/// Fabricate a minimal `.photoslibrary` bundle: a synthetic Core Data
	/// database plus a derivatives tree, enough to exercise the whole harvest.
	fn synthetic_library(dir: &Path) -> PathBuf {
		let library = dir.join("Test.photoslibrary");
		std::fs::create_dir_all(library.join("database")).unwrap();

		let conn = Connection::open(library.join("database/Photos.sqlite")).unwrap();
		conn.execute_batch(
			"CREATE TABLE ZASSET (
				Z_PK INTEGER PRIMARY KEY, ZUUID TEXT, ZDIRECTORY TEXT, ZFILENAME TEXT,
				ZDATECREATED REAL, ZMODIFICATIONDATE REAL, ZKIND INTEGER,
				ZWIDTH INTEGER, ZHEIGHT INTEGER, ZFAVORITE INTEGER, ZHIDDEN INTEGER,
				ZAVALANCHEUUID TEXT
			);
			CREATE TABLE ZADDITIONALASSETATTRIBUTES (ZASSET INTEGER, ZORIGINALFILENAME TEXT);
			CREATE TABLE ZEXTENDEDATTRIBUTES (
				ZASSET INTEGER, ZCAMERAMAKE TEXT, ZCAMERAMODEL TEXT, ZLENSMODEL TEXT,
				ZISO INTEGER, ZAPERTURE REAL, ZSHUTTERSPEED REAL, ZFOCALLENGTH REAL,
				ZFOCALLENGTHIN35MM INTEGER, ZEXPOSUREBIAS REAL, ZFLASHFIRED INTEGER,
				ZLATITUDE REAL, ZLONGITUDE REAL
			);
			CREATE TABLE ZGENERICALBUM (
				Z_PK INTEGER PRIMARY KEY, ZUUID TEXT, ZTITLE TEXT, ZKIND INTEGER,
				ZTRASHEDSTATE INTEGER
			);
			CREATE TABLE Z_28ASSETS (Z_28ALBUMS INTEGER, Z_3ASSETS INTEGER);
			CREATE TABLE ZDETECTEDFACE (ZPERSONFORFACE INTEGER, ZASSETFORFACE INTEGER);
			CREATE TABLE ZPERSON (Z_PK INTEGER PRIMARY KEY, ZPERSONUUID TEXT, ZFULLNAME TEXT);

			INSERT INTO ZASSET VALUES
				(1, 'AAAA-1111', 'A/B', 'IMG_0001.HEIC', 700000000.0, 700000100.0, 0,
				 4032, 3024, 1, 0, NULL),
				(2, 'BBBB-2222', 'B/C', 'IMG_0002.MOV', 710000000.0, NULL, 1,
				 1920, 1080, 0, 1, NULL),
				(3, 'CCCC-3333', 'C/D', 'IMG_0003.JPG', 720000000.0, NULL, 0,
				 6000, 4000, 0, 0, 'burst-1');

			INSERT INTO ZADDITIONALASSETATTRIBUTES VALUES
				(1, 'IMG_0001.HEIC'), (2, 'IMG_0002.MOV'), (3, 'IMG_0003.JPG');

			INSERT INTO ZEXTENDEDATTRIBUTES VALUES
				(1, 'Apple', 'iPhone 15 Pro', 'Main Camera', 100, 1.8, 0.004, 6.9, 24,
				 0.0, 0, 47.6, -122.3),
				(3, 'Sony', 'ILCE-7M4', '----', 400, 0.0, 0.002, 0.0, 0,
				 -0.3, 1, -180.0, -180.0);

			INSERT INTO ZGENERICALBUM VALUES
				(1, 'ALB-1', 'Trip', 2, 0),
				(2, 'ALB-2', 'Deleted', 2, 1),
				(3, 'ALB-3', 'Smart', 4000, 0),
				(4, 'ALB-4', 'Empty', 2, 0);

			INSERT INTO Z_28ASSETS VALUES (1, 1), (1, 2), (2, 3);

			INSERT INTO ZDETECTEDFACE VALUES (1, 1), (1, 1), (2, 2);
			INSERT INTO ZPERSON VALUES (1, 'P-1', 'Jamie Pine'), (2, 'P-2', NULL);",
		)
		.unwrap();

		// Derivatives: a full-size preview for asset 1, only a .THM for asset 2,
		// nothing for asset 3.
		let derivatives = library.join("resources/derivatives");
		std::fs::create_dir_all(derivatives.join("A")).unwrap();
		std::fs::create_dir_all(derivatives.join("B")).unwrap();
		std::fs::write(derivatives.join("A/AAAA-1111_1_105_c.jpeg"), b"jpeg").unwrap();
		std::fs::write(derivatives.join("B/BBBB-2222.THM"), b"thm").unwrap();

		library
	}

	#[test]
	fn schema_declares_photo_with_album_and_person_groupings() {
		let adapter = ApplePhotosAdapter::new();
		let schema = adapter.schema();
		assert_eq!(schema.data_type.id, "photo");
		assert!(schema.models.contains_key(PHOTO));
		assert!(schema.models.contains_key(ALBUM));
		assert!(schema.models.contains_key(PERSON));
		assert_eq!(schema.search.primary_model, PHOTO);
		assert_eq!(schema.search.date_field.as_deref(), Some("captured_at"));
	}

	#[test]
	fn original_path_is_projected_from_directory_and_filename() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let assets = read_assets(&library.join("database/Photos.sqlite")).unwrap();
		assert_eq!(assets.len(), 3);
		// Ordered by capture date.
		assert_eq!(assets[0].original_path, "originals/A/B/IMG_0001.HEIC");
		assert_eq!(assets[1].original_path, "originals/B/C/IMG_0002.MOV");
		assert_eq!(assets[2].burst_id.as_deref(), Some("burst-1"));
	}

	#[test]
	fn exif_placeholders_and_sentinels_are_dropped() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let assets = read_assets(&library.join("database/Photos.sqlite")).unwrap();
		let sony = &assets[2];
		assert_eq!(sony.camera_make.as_deref(), Some("Sony"));
		// "----" is a placeholder, 0.0 means "not recorded", -180 means "no fix".
		assert_eq!(sony.lens, None);
		assert_eq!(sony.aperture, None);
		assert_eq!(sony.focal_length, None);
		assert_eq!(sony.focal_length_35, None);
		assert_eq!(sony.gps_lat, None);
		assert_eq!(sony.gps_lon, None);
		assert_eq!(sony.flash, Some(true));

		let iphone = &assets[0];
		assert_eq!(iphone.gps_lat, Some(47.6));
		assert_eq!(iphone.aperture, Some(1.8));
	}

	#[test]
	fn album_join_resolves_by_shape_across_ordinals() {
		// The ordinal (28 here) shifts between Photos versions; resolution must
		// find the table by shape regardless.
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());
		let conn = Connection::open(library.join("database/Photos.sqlite")).unwrap();
		assert_eq!(
			resolve_album_join(&conn),
			Some((
				"Z_28ASSETS".to_string(),
				"Z_28ALBUMS".to_string(),
				"Z_3ASSETS".to_string()
			))
		);

		let other = Connection::open_in_memory().unwrap();
		other
			.execute_batch(
				"CREATE TABLE Z_33ASSETS (Z_33ALBUMS INTEGER, Z_3ASSETS INTEGER, Z_FOK_3ASSETS INTEGER);",
			)
			.unwrap();
		assert_eq!(
			resolve_album_join(&other),
			Some((
				"Z_33ASSETS".to_string(),
				"Z_33ALBUMS".to_string(),
				"Z_3ASSETS".to_string()
			))
		);

		let empty = Connection::open_in_memory().unwrap();
		assert_eq!(resolve_album_join(&empty), None);
	}

	#[test]
	fn groups_extract_named_albums_and_people() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let groups = read_groups(&library.join("database/Photos.sqlite")).unwrap();

		// One user album with members (trashed, smart, and empty albums are
		// skipped) and one named person (unnamed people are skipped).
		assert_eq!(groups.len(), 2);

		let album = groups.iter().find(|g| g.model == ALBUM).unwrap();
		assert_eq!(album.external_id, "apple:album:ALB-1");
		assert_eq!(album.title, "Trip");
		assert_eq!(album.member_zpks, vec![1, 2]);

		let person = groups.iter().find(|g| g.model == PERSON).unwrap();
		assert_eq!(person.external_id, "apple:person:P-1");
		assert_eq!(person.title, "Jamie Pine");
		// Two faces of the same person in asset 1 dedupe to one membership.
		assert_eq!(person.member_zpks, vec![1]);
	}

	#[test]
	fn thumbs_resolve_by_fallback_chain() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let mut assets = read_assets(&library.join("database/Photos.sqlite")).unwrap();
		resolve_thumbs(&mut assets, &library.join("resources"));

		assert_eq!(
			assets[0].thumb_path.as_deref(),
			Some("derivatives/A/AAAA-1111_1_105_c.jpeg")
		);
		assert_eq!(assets[0].thumb_px, Some(1180));
		assert!(assets[0].available);

		assert_eq!(
			assets[1].thumb_path.as_deref(),
			Some("derivatives/B/BBBB-2222.THM")
		);
		assert_eq!(assets[1].thumb_px, Some(32));

		assert_eq!(assets[2].thumb_path, None);
		assert!(!assets[2].available);
	}

	#[test]
	fn core_data_epoch_converts_to_unix_ms() {
		// 2001-01-01T00:00:00Z, the Core Data epoch.
		assert_eq!(to_unix_ms(0.0), 978_307_200_000);
		assert_eq!(
			rfc3339(978_307_200_000).as_deref(),
			Some("2001-01-01T00:00:00.000Z")
		);
	}

	/// Full sync against a real library copy, when one is provided. Point
	/// `SD_APPLE_PHOTOS_SMOKE_LIBRARY` at a `.photoslibrary` bundle (use a
	/// copy — the harvest is read-only, but keep the live library out of test
	/// runs); skipped otherwise.
	#[tokio::test]
	async fn smoke_syncs_a_real_library_copy() {
		let Ok(library) = std::env::var("SD_APPLE_PHOTOS_SMOKE_LIBRARY") else {
			return;
		};

		let dir = tempfile::tempdir().unwrap();
		let adapter = ApplePhotosAdapter::new();
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("photos-smoke", adapter.schema())
			.await
			.unwrap();
		let db = manager.open("photos-smoke").await.unwrap();
		db.begin_sync().await.unwrap();

		let config = json!({
			"library_path": library,
			"_data_dir": dir.path().join("sources/photos-smoke").to_string_lossy(),
		});
		let report = adapter.sync(&db, &config).await.unwrap();

		assert!(report.error.is_none());
		assert!(report.records_upserted > 0);
		println!(
			"smoke: {} records ({} photos, {} albums, {} people), {} membership edges in {}ms",
			report.records_upserted,
			db.count(PHOTO).await.unwrap(),
			db.count(ALBUM).await.unwrap(),
			db.count(PERSON).await.unwrap(),
			report.links_created,
			report.duration_ms,
		);
	}

	#[tokio::test]
	async fn sync_lands_photos_albums_and_people_in_the_record_table() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let adapter = ApplePhotosAdapter::new();
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("photos-test", adapter.schema())
			.await
			.unwrap();
		let db = manager.open("photos-test").await.unwrap();
		db.begin_sync().await.unwrap();

		let config = json!({
			"library_path": library.to_string_lossy(),
			"_data_dir": dir.path().join("sources/photos-test").to_string_lossy(),
		});
		let report = adapter.sync(&db, &config).await.unwrap();

		// 3 photos + 1 album + 1 person; album membership (2) + person (1).
		assert_eq!(report.records_upserted, 5);
		assert_eq!(report.links_created, 3);
		assert!(report.error.is_none());

		assert_eq!(db.count(PHOTO).await.unwrap(), 3);
		assert_eq!(db.count(ALBUM).await.unwrap(), 1);
		assert_eq!(db.count(PERSON).await.unwrap(), 1);

		// Facet locator paths are library-relative path evidence; the root
		// they resolve against travels in _sync_state.
		let original: String = sqlx::query_scalar(
			"SELECT f.original_path FROM facet_photo f
			 JOIN record r ON r.uuid = f.record_uuid WHERE r.external_id = 'AAAA-1111'",
		)
		.fetch_one(db.pool())
		.await
		.unwrap();
		assert_eq!(original, "originals/A/B/IMG_0001.HEIC");
		assert_eq!(
			db.get_cursor(crate::db::FILE_ROOT_CURSOR)
				.await
				.unwrap()
				.as_deref(),
			Some(library.to_string_lossy().as_ref())
		);
		let thumb: String = sqlx::query_scalar(
			"SELECT f.thumb_path FROM facet_photo f
			 JOIN record r ON r.uuid = f.record_uuid WHERE r.external_id = 'AAAA-1111'",
		)
		.fetch_one(db.pool())
		.await
		.unwrap();
		assert_eq!(thumb, "resources/derivatives/A/AAAA-1111_1_105_c.jpeg");

		// Membership edges: photo → album, typed by the grouping model.
		let album_members: i64 = sqlx::query_scalar(
			"SELECT COUNT(*) FROM edge e
			 JOIN record d ON d.uuid = e.dst_uuid
			 WHERE e.type = 'album' AND d.external_id = 'apple:album:ALB-1'",
		)
		.fetch_one(db.pool())
		.await
		.unwrap();
		assert_eq!(album_members, 2);

		// A second sync converges: no duplicate records, membership rebuilt.
		db.begin_sync().await.unwrap();
		let report = adapter.sync(&db, &config).await.unwrap();
		assert_eq!(report.records_upserted, 5);
		assert_eq!(db.count_all().await.unwrap(), 5);

		let edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edge")
			.fetch_one(db.pool())
			.await
			.unwrap();
		assert_eq!(edges, 3);
	}
}
