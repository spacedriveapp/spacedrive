//! # sd-photos — reading an Apple Photos library
//!
//! Reads a `.photoslibrary` bundle and returns what Apple knows about it:
//! assets with their EXIF and their library-relative original paths, the user's
//! albums, and named people with their asset membership.
//!
//! This crate reads. It does not decide what any of it becomes. Under
//! `docs/core/design/file-backed-sources.md` the filesystem source owns the
//! record for a photo — the bytes are already on disk and already indexed — and
//! what comes out of here is enrichment joined onto those records: places,
//! faces, albums. Nothing in this crate mints a record.
//!
//! Every Apple-specific decoding decision stays inside: the Core Data epoch,
//! the derivative fallback chain, the join tables named after entity ordinals,
//! the sentinel values that mean "not recorded". Callers get ordinary units.
//!
//! The read is strictly read-only. The catalog is snapshotted before any query
//! runs, originals and derivatives are referenced by path rather than copied,
//! and nothing is written back to the library.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// Best-thumbnail fallback chain: (filename suffix, pixel tier).
const THUMB_CHAIN: &[(&str, i64)] = &[
	("_1_105_c.jpeg", 1180),
	("_1_102_o.jpeg", 576),
	("_1_100_o.jpeg", 576),
	(".THM", 32),
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("no Photos.sqlite at {0} — is the library path correct?")]
	NoCatalog(PathBuf),
	#[error("no home directory to derive the default library path from")]
	NoHomeDir,
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether an asset is a still or a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
	Image,
	Video,
}

/// What kind of thing a [`Group`] gathers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
	/// A user-created album. Smart albums and trashed albums are not read.
	Album,
	/// A named person. Unnamed detected faces are not read.
	Person,
}

/// One asset as Apple has it, with its best derivative resolved.
#[derive(Debug, Clone)]
pub struct Asset {
	/// Core Data primary key. Album and face membership is expressed in these,
	/// so it is what [`Group::member_zpks`] joins against.
	pub z_pk: i64,
	/// Apple's stable identifier for the asset.
	pub zuuid: String,
	pub original_filename: Option<String>,
	/// Library-relative original location: `originals/<ZDIRECTORY>/<ZFILENAME>`.
	/// This is the path evidence that joins the asset to a filesystem record.
	pub original_path: String,
	/// Unix milliseconds. Apple stores seconds since 2001-01-01 UTC.
	pub captured_at_ms: i64,
	pub modified_at_ms: Option<i64>,
	pub media: MediaKind,
	pub width: Option<i64>,
	pub height: Option<i64>,
	pub favorite: bool,
	pub hidden: bool,
	/// Set when the asset is one frame of a burst.
	pub burst_id: Option<String>,
	// EXIF, from ZEXTENDEDATTRIBUTES. Stored directly — aperture is the
	// f-number and shutter is exposure seconds, so no APEX conversion.
	// Placeholders and sentinels are dropped rather than passed on.
	pub camera_make: Option<String>,
	pub camera_model: Option<String>,
	pub lens: Option<String>,
	pub iso: Option<i64>,
	pub aperture: Option<f64>,
	pub shutter: Option<f64>,
	pub focal_length: Option<f64>,
	pub focal_length_35: Option<i64>,
	pub exposure_bias: Option<f64>,
	pub flash: Option<bool>,
	pub gps_lat: Option<f64>,
	pub gps_lon: Option<f64>,
	/// Resources-relative derivative location, when one was found.
	pub thumb_path: Option<String>,
	/// The pixel tier of the derivative that was found.
	pub thumb_px: Option<i64>,
	/// Whether a derivative exists locally. False means the asset is in iCloud
	/// and not materialized on this machine.
	pub available: bool,
}

/// An album or a named person, with its members identified by [`Asset::z_pk`].
#[derive(Debug, Clone)]
pub struct Group {
	pub kind: GroupKind,
	/// Stable across syncs: `apple:album:<uuid>` or `apple:person:<uuid>`.
	pub external_id: String,
	pub title: String,
	pub member_zpks: Vec<i64>,
}

/// Everything one pass over a library yields.
#[derive(Debug, Clone)]
pub struct Harvest {
	pub assets: Vec<Asset>,
	pub groups: Vec<Group>,
}

/// The system library, when there is a home directory to find it under.
pub fn default_library_path() -> Result<PathBuf> {
	dirs::home_dir()
		.map(|home| home.join("Pictures/Photos Library.photoslibrary"))
		.ok_or(Error::NoHomeDir)
}

/// Read a library: snapshot the catalog, project assets, resolve derivatives,
/// extract groups.
///
/// Blocking. `snapshot_dir` is where the catalog copy lives; the caller owns
/// its lifetime. Call this off an async executor.
pub fn harvest(library: &Path, snapshot_dir: &Path) -> Result<Harvest> {
	let copy = snapshot_catalog(&library.join("database/Photos.sqlite"), snapshot_dir)?;
	let mut assets = read_assets(&copy)?;
	resolve_thumbs(&mut assets, &library.join("resources"));
	let groups = read_groups(&copy)?;
	Ok(Harvest { assets, groups })
}

/// Snapshot one file. On APFS a clone is instant and consumes no space until
/// the original diverges — a multi-gigabyte catalog costs nothing per pass.
/// Filesystems without cloning fall back to a byte copy.
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

/// Copy `Photos.sqlite` with its WAL and SHM sidecars, so queries run against a
/// stable snapshot without ever touching the live library's WAL.
fn snapshot_catalog(src_db: &Path, dir: &Path) -> Result<PathBuf> {
	if !src_db.exists() {
		return Err(Error::NoCatalog(src_db.to_path_buf()));
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

/// Project the Core Data schema into [`Asset`]s, ordered by capture date.
fn read_assets(catalog: &Path) -> Result<Vec<Asset>> {
	let conn = Connection::open_with_flags(catalog, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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
			captured_at_ms: to_unix_ms(r.get::<_, Option<f64>>(3)?.unwrap_or(0.0)),
			modified_at_ms: r.get::<_, Option<f64>>(23)?.map(to_unix_ms),
			media: media_kind(r.get::<_, Option<i64>>(4)?.unwrap_or(0)),
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
/// Photos — they are a map over per-asset coordinates, which travel on the
/// asset itself as `gps_lat`/`gps_lon`.
fn read_groups(catalog: &Path) -> Result<Vec<Group>> {
	let conn = Connection::open_with_flags(catalog, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
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
			out.push(Group {
				kind: GroupKind::Album,
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
			out.push(Group {
				kind: GroupKind::Person,
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

/// Map Apple's `ZKIND`.
fn media_kind(kind: i64) -> MediaKind {
	match kind {
		1 => MediaKind::Video,
		_ => MediaKind::Image,
	}
}

/// Core Data seconds-since-2001 → unix milliseconds.
fn to_unix_ms(core_data_seconds: f64) -> i64 {
	((core_data_seconds + 978_307_200.0) * 1000.0) as i64
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

	/// Fabricate a minimal `.photoslibrary` bundle: a synthetic Core Data
	/// database plus a derivatives tree, enough to exercise the whole read.
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
	fn media_kind_and_timestamps_come_back_in_ordinary_units() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());

		let assets = read_assets(&library.join("database/Photos.sqlite")).unwrap();
		assert_eq!(assets[0].media, MediaKind::Image);
		assert_eq!(assets[1].media, MediaKind::Video);

		// Core Data seconds since 2001-01-01 become unix milliseconds.
		assert_eq!(assets[0].captured_at_ms, (700_000_000 + 978_307_200) * 1000);
		assert_eq!(
			assets[0].modified_at_ms,
			Some((700_000_100 + 978_307_200) * 1000)
		);
		assert_eq!(assets[1].modified_at_ms, None);
		assert_eq!(to_unix_ms(0.0), 978_307_200_000);
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

		let album = groups
			.iter()
			.find(|g| g.kind == GroupKind::Album)
			.expect("an album");
		assert_eq!(album.external_id, "apple:album:ALB-1");
		assert_eq!(album.title, "Trip");
		assert_eq!(album.member_zpks, vec![1, 2]);

		let person = groups
			.iter()
			.find(|g| g.kind == GroupKind::Person)
			.expect("a person");
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
	fn harvest_snapshots_the_catalog_and_leaves_the_library_alone() {
		let dir = tempfile::tempdir().unwrap();
		let library = synthetic_library(dir.path());
		let snapshot = dir.path().join("snapshot");

		let out = harvest(&library, &snapshot).unwrap();
		assert_eq!(out.assets.len(), 3);
		assert_eq!(out.groups.len(), 2);
		assert!(out.assets[0].available);

		// The copy is what was queried, and the live catalog is untouched.
		assert!(snapshot.join("Photos.sqlite").exists());
		assert!(library.join("database/Photos.sqlite").exists());

		// A second pass re-copies rather than reusing a stale snapshot.
		let again = harvest(&library, &snapshot).unwrap();
		assert_eq!(again.assets.len(), 3);
	}

	#[test]
	fn a_missing_catalog_is_named_in_the_error() {
		let dir = tempfile::tempdir().unwrap();
		let err = harvest(&dir.path().join("Nope.photoslibrary"), dir.path()).unwrap_err();
		assert!(matches!(err, Error::NoCatalog(_)));
	}
}
