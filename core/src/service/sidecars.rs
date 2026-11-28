//! # Sidecar registry
//!
//! Everything derived from a file's bytes, other than thumbnail tiles and
//! scrub sheets, is a content sidecar: a proxy, speech audio, a transcript, a
//! shot list, a keyframe, or a document an extension wrote. Each one is a file
//! in a source's `derived/` directory and a row in that source's
//! `content_sidecar` table, and [`SidecarRegistry::publish`] is the one path
//! that makes both: it renames the writer's partial file into place, records
//! the row and announces it.
//!
//! A content has one copy, under the source whose record produced it. When a
//! second source holds the same bytes, the producer finds the first copy and
//! adds to it rather than making another, and a reader that knows only the
//! content uuid probes every source of the library, which [`SidecarFinder`]
//! does without touching a database.
//!
//! The file is the authority and the row is the index of it. A row that
//! failed to write, or a store that was away, is repaired by
//! [`SidecarRegistry::rebuild`], which lists the directory, and a row whose
//! file is gone is dropped by the same pass. Nothing here re-derives an
//! artifact, since a transcript can cost minutes of model time.
//!
//! Thumbnail tiles stay record-keyed in `sidecars.db` and their hot tier in
//! the volume's `thumbs.pvcache`; `docs/core/design/thumbnail-storage.md`
//! explains why. `files.sidecars` reports them beside what is listed here.

use std::{
	fs, io,
	path::{Path, PathBuf},
	sync::Arc,
	time::{SystemTime, UNIX_EPOCH},
};

use sd_store::{ContentSidecarRow, SourceDb};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
	domain::sidecar::ContentSidecar,
	infra::event::{Event, EventBus},
	library::Library,
	ops::indexing::VolumeIndex,
};

/// The version recorded for artifacts the core makes.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The directories that may hold a library's content sidecars, in the order
/// a reader probes them: each source's `derived/`, then the tree the library
/// kept before the move, until it is adopted.
#[derive(Debug, Clone, Default)]
pub struct SidecarFinder {
	roots: Vec<PathBuf>,
}

impl SidecarFinder {
	pub fn new(roots: Vec<PathBuf>) -> Self {
		Self { roots }
	}

	/// The file holding `sidecar`, wherever it is.
	pub fn find(&self, sidecar: &ContentSidecar) -> Option<PathBuf> {
		let relative = sidecar.relative_path();
		self.roots
			.iter()
			.map(|root| root.join(&relative))
			.find(|path| path.is_file())
	}
}

/// Where one content's sidecars are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
	pub source: Uuid,
	/// The source's own root, which names its store.
	pub source_root: PathBuf,
	/// The source's `derived/` directory.
	pub root: PathBuf,
}

impl Home {
	pub fn path(&self, sidecar: &ContentSidecar) -> PathBuf {
		self.root.join(sidecar.relative_path())
	}

	/// Where a writer fills `sidecar` before publishing it.
	pub fn partial(&self, sidecar: &ContentSidecar) -> PathBuf {
		sidecar.partial_path(&self.root)
	}
}

/// One content sidecar as [`SidecarRegistry::list`] finds it.
#[derive(Debug, Clone)]
pub struct Listed {
	pub sidecar: ContentSidecar,
	pub path: PathBuf,
	pub size: u64,
	/// Modification time in milliseconds, which a client's URL carries so an
	/// immutable response is fetched again after a rewrite.
	pub modified_ms: i64,
}

/// What [`SidecarRegistry::adopt`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Adopted {
	/// Contents whose directory now sits under a source.
	pub moved: u64,
	/// Contents no source of the library holds, left where they were.
	pub left: u64,
}

#[derive(Debug, Clone)]
struct Source {
	id: Uuid,
	root: PathBuf,
	derived: PathBuf,
}

/// A library's content sidecars across its sources.
pub struct SidecarRegistry {
	library_id: Uuid,
	library_dir: PathBuf,
	volume_index: Arc<VolumeIndex>,
	events: Arc<EventBus>,
}

impl SidecarRegistry {
	pub fn of(library: &Library) -> Self {
		let context = library.core_context();
		Self {
			library_id: library.id(),
			library_dir: library.path().to_path_buf(),
			volume_index: context.volume_index().clone(),
			events: context.events.clone(),
		}
	}

	fn sources(&self) -> Vec<Source> {
		let Some(dirs) = self.volume_index.source_dirs() else {
			return Vec::new();
		};
		let mut sources: Vec<Source> = self
			.volume_index
			.sources_of(self.library_id)
			.into_iter()
			.map(|source| Source {
				id: source.id,
				derived: sd_sidecar_path::derived_root(dirs.root(), &source.id),
				root: source.root,
			})
			.collect();
		sources.sort_by_key(|source| source.id);
		sources
	}

	pub fn finder(&self) -> SidecarFinder {
		let mut roots: Vec<PathBuf> = self
			.sources()
			.into_iter()
			.map(|source| source.derived)
			.collect();
		roots.push(sd_sidecar_path::legacy_root(&self.library_dir));
		SidecarFinder::new(roots)
	}

	/// Where `content_uuid`'s sidecars are written: the source that already
	/// holds a copy, else the source owning `origin`, the file they are being
	/// made from, else any source whose store holds the content.
	pub async fn home(&self, content_uuid: Uuid, origin: Option<&Path>) -> io::Result<Home> {
		let sources = self.sources();
		let directory = sd_sidecar_path::content_directory(&content_uuid);
		let existing: Vec<Uuid> = sources
			.iter()
			.filter(|source| source.derived.join(&directory).is_dir())
			.map(|source| source.id)
			.collect();
		let origin = origin
			.and_then(|path| self.volume_index.source_id_for(path))
			.filter(|id| sources.iter().any(|source| source.id == *id));
		let mut holders = Vec::new();
		if existing.is_empty() && origin.is_none() {
			for source in &sources {
				if let Some(db) = self.volume_index.read_store(source.id).await {
					if db.holds_content(content_uuid).await.unwrap_or(false) {
						holders.push(source.id);
					}
				}
			}
		}
		let chosen = choose_home(&existing, origin, &holders).ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::NotFound,
				format!("no source of this library holds content {content_uuid}"),
			)
		})?;
		let source = sources
			.into_iter()
			.find(|source| source.id == chosen)
			.expect("the chosen source is one of the library's");
		mark(&source.derived, self.library_id).await?;
		Ok(Home {
			source: source.id,
			source_root: source.root,
			root: source.derived,
		})
	}

	/// Rename a finished partial file into place as `sidecar`, record its row
	/// and announce it. `version` is the producer's version, the core's when
	/// `None`.
	///
	/// The file is published even when the row cannot be written, such as
	/// for a store on a drive that went away mid-run; the next rebuild lists
	/// it. Answers the published path.
	pub async fn publish(
		&self,
		home: &Home,
		sidecar: &ContentSidecar,
		partial: &Path,
		version: Option<&str>,
	) -> io::Result<PathBuf> {
		let store = self.volume_index.store_for(&home.source_root).await;
		let (path, row) = publish_into(
			store.as_ref().map(|store| store.db()),
			&home.root,
			sidecar,
			partial,
			version,
		)
		.await?;
		if row.is_none() {
			warn!(source = %home.source, path = %path.display(), "published a sidecar without its row");
		}
		self.announce(home.source, sidecar, "published");
		Ok(path)
	}

	/// Delete `sidecar` wherever it is, file and row. Answers whether a file
	/// was there.
	pub async fn remove(&self, sidecar: &ContentSidecar) -> io::Result<bool> {
		let relative = sidecar.relative_path();
		let mut removed = false;
		for source in self.sources() {
			let path = source.derived.join(&relative);
			match tokio::fs::remove_file(&path).await {
				Ok(()) => removed = true,
				Err(error) if error.kind() == io::ErrorKind::NotFound => {}
				Err(error) => return Err(error),
			}
			if let Some(store) = self.volume_index.store_for(&source.root).await {
				if let Err(error) = store.db().delete_content_sidecar(&sidecar.key()).await {
					warn!(source = %source.id, %error, "could not forget a removed sidecar");
				}
			}
			if removed {
				self.announce(source.id, sidecar, "removed");
			}
		}
		let legacy = sd_sidecar_path::legacy_root(&self.library_dir).join(&relative);
		match tokio::fs::remove_file(&legacy).await {
			Ok(()) => removed = true,
			Err(error) if error.kind() == io::ErrorKind::NotFound => {}
			Err(error) => return Err(error),
		}
		Ok(removed)
	}

	fn announce(&self, source: Uuid, sidecar: &ContentSidecar, change: &str) {
		self.events.emit(Event::ResourceChanged {
			resource_type: "content_sidecar".to_string(),
			resource: serde_json::json!({
				"change": change,
				"source_id": source,
				"content_uuid": sidecar.content_uuid,
				"producer": sidecar.producer(),
				"kind": sidecar.kind.as_str(),
				"variant": sidecar.variant.as_str(),
				"format": sidecar.format.extension(),
			}),
			metadata: None,
		});
	}

	/// Every content sidecar of `content_uuid` in the library, from each
	/// source's rows, or from its directory when it lists none, then from
	/// the legacy tree. The first copy of a key wins.
	pub async fn list(&self, content_uuid: Uuid) -> Vec<Listed> {
		let mut roots: Vec<(PathBuf, Vec<ContentSidecar>)> = Vec::new();
		for source in self.sources() {
			let rows = match self.volume_index.read_store(source.id).await {
				Some(db) => db
					.content_sidecars(content_uuid)
					.await
					.unwrap_or_else(|error| {
						warn!(source = %source.id, %error, "could not read content sidecars");
						Vec::new()
					}),
				None => Vec::new(),
			};
			let named = rows
				.iter()
				.filter_map(|row| ContentSidecar::from_key(&row.key()))
				.collect();
			roots.push((source.derived, named));
		}
		roots.push((sd_sidecar_path::legacy_root(&self.library_dir), Vec::new()));
		let directory = sd_sidecar_path::content_directory(&content_uuid);
		// Stat and directory walks block, so they run off the executor.
		tokio::task::spawn_blocking(move || {
			let mut found: Vec<Listed> = Vec::new();
			for (root, named) in roots {
				let mut listed: Vec<Listed> = named
					.into_iter()
					.filter_map(|sidecar| listed_at(&root, sidecar))
					.collect();
				if listed.is_empty() {
					listed = scan_tree(&root, &directory)
						.into_iter()
						.filter_map(|sidecar| listed_at(&root, sidecar))
						.collect();
				}
				merge(&mut found, listed);
			}
			found
		})
		.await
		.unwrap_or_else(|error| {
			warn!(%error, "content sidecar listing failed");
			Vec::new()
		})
	}

	/// Bring every source's rows in line with its directory.
	pub async fn rebuild(&self) {
		for source in self.sources() {
			let has_rows = match self.volume_index.read_store(source.id).await {
				Some(db) => db
					.content_sidecar_totals()
					.await
					.map_or(false, |(count, _)| count > 0),
				None => false,
			};
			if !has_rows && !source.derived.is_dir() {
				continue;
			}
			if let Err(error) = mark(&source.derived, self.library_id).await {
				warn!(source = %source.id, %error, "could not mark the derived directory");
			}
			let Some(store) = self.volume_index.store_for(&source.root).await else {
				continue;
			};
			match rebuild_root(&source.derived, store.db()).await {
				Ok((0, 0)) => {}
				Ok((added, removed)) => {
					info!(source = %source.id, added, removed, "rebuilt content sidecar rows")
				}
				Err(error) => {
					warn!(source = %source.id, %error, "could not rebuild content sidecar rows")
				}
			}
		}
	}

	/// Delete the sidecars of content no source of the library holds any
	/// more, files and rows.
	///
	/// A content row appears only after a file is identified, so an
	/// artifact made before that would look orphaned. Pruning therefore
	/// waits until every source is reachable and has identified everything
	/// it holds; a source that cannot answer could be holding the content.
	pub async fn prune(&self) -> u64 {
		let sources = self.sources();
		let mut stores = Vec::new();
		for source in &sources {
			let Some(db) = self.volume_index.read_store(source.id).await else {
				debug!(source = %source.id, "store unreachable; not pruning content sidecars");
				return 0;
			};
			// Files outside the deep scope are never identified, so waiting on them would stop pruning for good.
			let pending =
				match crate::ops::indexing::deep_scope::deep_scope(&db, &source.root).await {
					Ok(scope) => sd_store::count_files_needing_content(db.pool(), &scope).await,
					Err(error) => Err(error),
				};
			match pending {
				Ok(0) => stores.push(db),
				_ => {
					debug!(source = %source.id, "identification pending; not pruning content sidecars");
					return 0;
				}
			}
		}
		let mut pruned = 0;
		for source in &sources {
			let Some(store) = self.volume_index.store_for(&source.root).await else {
				continue;
			};
			match prune_root(&source.derived, store.db(), &stores).await {
				Ok(count) => pruned += count,
				Err(error) => {
					warn!(source = %source.id, %error, "could not prune content sidecars")
				}
			}
		}
		if pruned > 0 {
			info!(library = %self.library_id, pruned, "pruned sidecars of content no source holds");
		}
		pruned
	}

	/// Move the library's pre-source tree under the sources that hold each
	/// content. Every move is a rename, and a content is moved whole when
	/// its destination is empty, so stopping at any point leaves each file
	/// in exactly one of the two places; the next run picks up from there.
	/// A content no source holds yet stays where readers still find it.
	pub async fn adopt(&self) -> io::Result<Adopted> {
		let legacy = sd_sidecar_path::legacy_root(&self.library_dir);
		let contents = {
			let legacy = legacy.clone();
			tokio::task::spawn_blocking(move || legacy_contents(&legacy))
				.await
				.map_err(io::Error::other)??
		};
		let mut adopted = Adopted::default();
		for (content_uuid, directory) in contents {
			let home = match self.home(content_uuid, None).await {
				Ok(home) => home,
				Err(_) => {
					adopted.left += 1;
					continue;
				}
			};
			let destination = home
				.root
				.join(sd_sidecar_path::content_directory(&content_uuid));
			tokio::task::spawn_blocking(move || adopt_content(&directory, &destination))
				.await
				.map_err(io::Error::other)??;
			adopted.moved += 1;
		}
		let sidecars = self.library_dir.join("sidecars");
		tokio::task::spawn_blocking(move || {
			remove_empty_dirs(&legacy);
			let _ = fs::remove_dir(&legacy);
			let _ = fs::remove_dir(&sidecars);
		})
		.await
		.map_err(io::Error::other)?;
		Ok(adopted)
	}

	/// Before a source's tree is deleted, move each content another source
	/// still holds to that source, so deleting one copy of a file never takes
	/// the transcript of the others with it.
	pub async fn evacuate(&self, leaving: Uuid) -> io::Result<u64> {
		let sources = self.sources();
		let Some(from) = sources.iter().find(|source| source.id == leaving) else {
			return Ok(0);
		};
		let contents = {
			let root = from.derived.clone();
			tokio::task::spawn_blocking(move || legacy_contents(&root))
				.await
				.map_err(io::Error::other)??
		};
		let mut moved = 0;
		let mut touched = Vec::new();
		for (content_uuid, directory) in contents {
			// A holder that cannot be asked is not taken as absent, since the
			// leaving tree may be deleted once this returns.
			let mut target = None;
			for source in sources.iter().filter(|source| source.id != leaving) {
				let Some(db) = self.volume_index.read_store(source.id).await else {
					return Err(io::Error::other(format!(
						"source {} did not answer, so its shared sidecars cannot be placed",
						source.id
					)));
				};
				if db
					.holds_content(content_uuid)
					.await
					.map_err(io::Error::other)?
				{
					target = Some(source.clone());
					break;
				}
			}
			let Some(target) = target else {
				continue;
			};
			mark(&target.derived, self.library_id).await?;
			let destination = target
				.derived
				.join(sd_sidecar_path::content_directory(&content_uuid));
			tokio::task::spawn_blocking(move || adopt_content(&directory, &destination))
				.await
				.map_err(io::Error::other)??;
			moved += 1;
			touched.push(target);
		}
		for target in touched {
			if let Some(store) = self.volume_index.store_for(&target.root).await {
				rebuild_root(&target.derived, store.db()).await?;
			}
		}
		Ok(moved)
	}

	/// Adopt the legacy tree, rebuild rows from disk and prune, in that
	/// order, once a library's sources are registered.
	pub async fn settle(self) {
		match self.adopt().await {
			Ok(adopted) if adopted.moved > 0 || adopted.left > 0 => info!(
				library = %self.library_id,
				moved = adopted.moved,
				left = adopted.left,
				"adopted the library's sidecar tree into its sources"
			),
			Ok(_) => {}
			Err(error) => {
				warn!(library = %self.library_id, %error, "could not adopt the sidecar tree")
			}
		}
		self.rebuild().await;
		self.prune().await;
	}
}

/// How many content sidecars a library's sources list and their total size,
/// plus whatever its legacy tree still holds. Library statistics report it.
pub async fn sidecar_totals(
	volume_index: &VolumeIndex,
	library_id: Uuid,
	library_dir: &Path,
) -> (u64, u64) {
	let (mut count, mut size) = (0, 0);
	for source in volume_index.sources_of(library_id) {
		if let Some(db) = volume_index.read_store(source.id).await {
			if let Ok((c, s)) = db.content_sidecar_totals().await {
				count += c;
				size += s;
			}
		}
	}
	let legacy = sd_sidecar_path::legacy_root(library_dir);
	let (c, s) = tokio::task::spawn_blocking(move || tree_totals(&legacy))
		.await
		.unwrap_or((0, 0));
	(count + c, size + s)
}

/// The one rule for where a content's sidecars go: an existing copy, the
/// origin's own when it holds one, then the origin, then a source holding
/// the content.
pub(crate) fn choose_home(
	existing: &[Uuid],
	origin: Option<Uuid>,
	holders: &[Uuid],
) -> Option<Uuid> {
	if let Some(origin) = origin.filter(|origin| existing.contains(origin)) {
		return Some(origin);
	}
	existing
		.first()
		.copied()
		.or(origin)
		.or_else(|| holders.first().copied())
}

/// Name the library in a source's derived directory, for the HTTP sidecar
/// routes, which see only the data directory.
async fn mark(derived: &Path, library_id: Uuid) -> io::Result<()> {
	let derived = derived.to_path_buf();
	tokio::task::spawn_blocking(move || sd_sidecar_path::mark_library(&derived, &library_id))
		.await
		.map_err(io::Error::other)?
}

fn merge(found: &mut Vec<Listed>, listed: Vec<Listed>) {
	for item in listed {
		if !found.iter().any(|seen| seen.sidecar == item.sidecar) {
			found.push(item);
		}
	}
}

fn listed_at(root: &Path, sidecar: ContentSidecar) -> Option<Listed> {
	let path = root.join(sidecar.relative_path());
	let metadata = fs::metadata(&path).ok().filter(|m| m.is_file())?;
	Some(Listed {
		sidecar,
		size: metadata.len(),
		modified_ms: metadata
			.modified()
			.ok()
			.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
			.map_or(0, |since| since.as_millis() as i64),
		path,
	})
}

/// The sidecars under `root/within`, named by their paths relative to
/// `root`. Partial files and anything the layout does not name are skipped.
fn scan_tree(root: &Path, within: &Path) -> Vec<ContentSidecar> {
	let mut found = Vec::new();
	let mut pending = vec![root.join(within)];
	while let Some(directory) = pending.pop() {
		let Ok(entries) = fs::read_dir(&directory) else {
			continue;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			match entry.file_type() {
				Ok(kind) if kind.is_dir() => pending.push(path),
				Ok(kind) if kind.is_file() => {
					if let Some(sidecar) = path
						.strip_prefix(root)
						.ok()
						.and_then(ContentSidecar::from_relative_path)
					{
						found.push(sidecar);
					}
				}
				_ => {}
			}
		}
	}
	found
}

fn now_ms() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |since| since.as_millis() as i64)
}

/// Size and BLAKE3 of a file, off the async runtime.
async fn measure(path: &Path) -> io::Result<(u64, String)> {
	let path = path.to_path_buf();
	tokio::task::spawn_blocking(move || {
		let mut hasher = blake3::Hasher::new();
		hasher.update_reader(fs::File::open(&path)?)?;
		Ok((
			fs::metadata(&path)?.len(),
			hasher.finalize().to_hex().to_string(),
		))
	})
	.await
	.map_err(io::Error::other)?
}

/// Write `sidecar`'s row from the file under `root`.
async fn record_file(
	db: &SourceDb,
	root: &Path,
	sidecar: &ContentSidecar,
	version: Option<&str>,
) -> io::Result<ContentSidecarRow> {
	let (size, checksum) = measure(&root.join(sidecar.relative_path())).await?;
	let key = sidecar.key();
	let row = ContentSidecarRow {
		content_uuid: key.content_uuid,
		producer: key.producer,
		kind: key.kind,
		variant: key.variant,
		format: key.format,
		producer_version: Some(version.unwrap_or(CORE_VERSION).to_string()),
		size,
		checksum: Some(checksum),
		created_at: now_ms(),
	};
	db.put_content_sidecar(&row)
		.await
		.map_err(|error| io::Error::other(error.to_string()))?;
	Ok(row)
}

/// The rename and row of [`SidecarRegistry::publish`], against one root and
/// store.
pub(crate) async fn publish_into(
	db: Option<&SourceDb>,
	root: &Path,
	sidecar: &ContentSidecar,
	partial: &Path,
	version: Option<&str>,
) -> io::Result<(PathBuf, Option<ContentSidecarRow>)> {
	let path = root.join(sidecar.relative_path());
	if let Some(parent) = path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	tokio::fs::rename(partial, &path).await?;
	let row = match db {
		Some(db) => match record_file(db, root, sidecar, version).await {
			Ok(row) => Some(row),
			Err(error) => {
				warn!(path = %path.display(), %error, "could not record a published sidecar");
				None
			}
		},
		None => None,
	};
	Ok((path, row))
}

/// List every file under `root` that has no row, and drop every row whose
/// file is gone. Answers how many rows were added and removed.
pub(crate) async fn rebuild_root(root: &Path, db: &SourceDb) -> io::Result<(u64, u64)> {
	let rows = db
		.all_content_sidecars()
		.await
		.map_err(|error| io::Error::other(error.to_string()))?;
	let on_disk = {
		let root = root.to_path_buf();
		tokio::task::spawn_blocking(move || scan_tree(&root, Path::new("")))
			.await
			.map_err(io::Error::other)?
	};
	let listed: std::collections::HashSet<_> = rows.iter().map(|row| row.key()).collect();
	let present: std::collections::HashSet<_> =
		on_disk.iter().map(|sidecar| sidecar.key()).collect();

	let mut added = 0;
	for sidecar in &on_disk {
		if !listed.contains(&sidecar.key()) {
			record_file(db, root, sidecar, None).await?;
			added += 1;
		}
	}
	let mut removed = 0;
	for row in &rows {
		if !present.contains(&row.key()) {
			db.delete_content_sidecar(&row.key())
				.await
				.map_err(|error| io::Error::other(error.to_string()))?;
			removed += 1;
		}
	}
	Ok((added, removed))
}

/// Delete the files and rows under `root` of content `db` lists sidecars
/// for but holds no content row for, unless one of `stores` holds it.
pub(crate) async fn prune_root(
	root: &Path,
	db: &SourceDb,
	stores: &[Arc<SourceDb>],
) -> io::Result<u64> {
	let orphans = db
		.content_sidecar_orphans()
		.await
		.map_err(|error| io::Error::other(error.to_string()))?;
	let mut pruned = 0;
	'orphan: for content_uuid in orphans {
		for store in stores {
			if store.holds_content(content_uuid).await.unwrap_or(true) {
				continue 'orphan;
			}
		}
		let directory = root.join(sd_sidecar_path::content_directory(&content_uuid));
		match tokio::fs::remove_dir_all(&directory).await {
			Ok(()) => {}
			Err(error) if error.kind() == io::ErrorKind::NotFound => {}
			Err(error) => return Err(error),
		}
		db.delete_content_sidecars_of(content_uuid)
			.await
			.map_err(|error| io::Error::other(error.to_string()))?;
		pruned += 1;
	}
	if pruned > 0 {
		let root = root.to_path_buf();
		tokio::task::spawn_blocking(move || remove_empty_dirs(&root))
			.await
			.map_err(io::Error::other)?;
	}
	Ok(pruned)
}

/// Every content directory in a tree laid out as `{h0}/{h1}/{content_uuid}`.
fn legacy_contents(root: &Path) -> io::Result<Vec<(Uuid, PathBuf)>> {
	let mut found = Vec::new();
	let shards = match fs::read_dir(root) {
		Ok(entries) => entries,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(found),
		Err(error) => return Err(error),
	};
	for h0 in shards.flatten().filter(|e| e.path().is_dir()) {
		for h1 in fs::read_dir(h0.path())?
			.flatten()
			.filter(|e| e.path().is_dir())
		{
			for content in fs::read_dir(h1.path())?.flatten() {
				let Some(uuid) = content
					.file_name()
					.to_str()
					.and_then(|name| Uuid::parse_str(name).ok())
				else {
					continue;
				};
				if content.path().is_dir() {
					found.push((uuid, content.path()));
				}
			}
		}
	}
	found.sort();
	Ok(found)
}

/// Move one content's directory to `destination`. An empty destination
/// takes the directory in one rename. Otherwise each file moves on its own,
/// a file the destination already holds is the same artifact and the source
/// copy is dropped, and a writer's leftover partial file is deleted. Running
/// it again after an interruption finishes the job.
pub(crate) fn adopt_content(source: &Path, destination: &Path) -> io::Result<()> {
	if !source.exists() {
		return Ok(());
	}
	if !destination.exists() {
		if let Some(parent) = destination.parent() {
			fs::create_dir_all(parent)?;
		}
		match fs::rename(source, destination) {
			Ok(()) => return Ok(()),
			Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
			Err(error) => return Err(error),
		}
	}
	let mut pending = vec![source.to_path_buf()];
	while let Some(directory) = pending.pop() {
		for entry in fs::read_dir(&directory)?.flatten() {
			let path = entry.path();
			if entry.file_type()?.is_dir() {
				pending.push(path);
				continue;
			}
			let partial = entry.file_name().to_string_lossy().starts_with('.');
			let target = destination.join(path.strip_prefix(source).expect("walked from source"));
			if partial || target.exists() {
				fs::remove_file(&path)?;
				continue;
			}
			if let Some(parent) = target.parent() {
				fs::create_dir_all(parent)?;
			}
			move_file(&path, &target)?;
		}
	}
	remove_empty_dirs(source);
	let _ = fs::remove_dir(source);
	Ok(())
}

/// A rename, or a copy through a dot file and a rename when the two paths
/// are on different volumes.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
	match fs::rename(from, to) {
		Ok(()) => return Ok(()),
		Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {}
		Err(error) => return Err(error),
	}
	let name = to
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let staging = to.with_file_name(format!(".{name}.adopting"));
	fs::copy(from, &staging)?;
	fs::File::open(&staging)?.sync_all()?;
	fs::rename(&staging, to)?;
	fs::remove_file(from)
}

/// Remove every empty directory beneath `root`, deepest first.
fn remove_empty_dirs(root: &Path) {
	let Ok(entries) = fs::read_dir(root) else {
		return;
	};
	for entry in entries.flatten() {
		if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
			let path = entry.path();
			remove_empty_dirs(&path);
			let _ = fs::remove_dir(&path);
		}
	}
}

fn tree_totals(root: &Path) -> (u64, u64) {
	let (mut count, mut size) = (0, 0);
	let mut pending = vec![root.to_path_buf()];
	while let Some(directory) = pending.pop() {
		let Ok(entries) = fs::read_dir(&directory) else {
			continue;
		};
		for entry in entries.flatten() {
			match entry.metadata() {
				Ok(metadata) if metadata.is_dir() => pending.push(entry.path()),
				Ok(metadata) => {
					count += 1;
					size += metadata.len();
				}
				Err(_) => {}
			}
		}
	}
	(count, size)
}

/// Starts [`SidecarRegistry::settle`] in the background for a library that
/// just opened.
pub fn settle_in_background(library: &Library) {
	let registry = SidecarRegistry::of(library);
	tokio::spawn(registry.settle());
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::domain::sidecar::{ProxyResolution, SidecarFormat};

	async fn store(dir: &Path, name: &str) -> SourceDb {
		let manager = sd_store::SourceManager::new(dir.to_path_buf());
		manager
			.create(name, &sd_store::filesystem_schema())
			.await
			.expect("source created");
		manager.open(name).await.expect("open")
	}

	async fn hold(db: &SourceDb, content_uuid: Uuid) {
		sqlx::query("INSERT INTO content (uuid, candidate_uuid, sampled_hash) VALUES (?, ?, ?)")
			.bind(Uuid::now_v7())
			.bind(content_uuid)
			.bind(content_uuid.to_string())
			.execute(db.pool())
			.await
			.unwrap();
	}

	fn write(path: &Path, bytes: &[u8]) {
		fs::create_dir_all(path.parent().unwrap()).unwrap();
		fs::write(path, bytes).unwrap();
	}

	#[tokio::test]
	async fn publish_renames_the_partial_and_records_its_row() {
		let dir = tempfile::tempdir().unwrap();
		let db = store(dir.path(), "a").await;
		let root = dir.path().join("derived");
		let sidecar = ContentSidecar::shots(Uuid::now_v7());
		let partial = sidecar.partial_path(&root);
		write(&partial, b"{\"shots\":[]}");

		let (path, row) = publish_into(Some(&db), &root, &sidecar, &partial, None)
			.await
			.unwrap();
		assert_eq!(path, root.join(sidecar.relative_path()));
		assert!(path.is_file() && !partial.exists());
		let row = row.expect("a row");
		assert_eq!(row.size, 12);
		assert_eq!(row.producer, "core");
		assert_eq!(
			row.checksum.as_deref(),
			Some(blake3::hash(b"{\"shots\":[]}").to_hex().as_str())
		);
		assert_eq!(
			db.content_sidecars(sidecar.content_uuid).await.unwrap(),
			vec![row]
		);
	}

	#[tokio::test]
	async fn rebuild_lists_files_without_rows_and_drops_rows_without_files() {
		let dir = tempfile::tempdir().unwrap();
		let db = store(dir.path(), "a").await;
		let root = dir.path().join("derived");
		let content = Uuid::now_v7();
		let proxy = ContentSidecar::proxy(content, ProxyResolution::P720);
		let transcript = ContentSidecar::transcript(content, SidecarFormat::Json);
		write(&root.join(proxy.relative_path()), b"mp4");
		write(&root.join(transcript.relative_path()), b"{}");
		write(&transcript.partial_path(&root), b"{");
		let faces = ContentSidecar {
			content_uuid: content,
			kind: crate::domain::sidecar::SidecarKind::extension("com.spacedrive.photos", "faces")
				.unwrap(),
			variant: "default".into(),
			format: SidecarFormat::Json,
		};
		write(&root.join(faces.relative_path()), b"[]");
		let gone = ContentSidecar::shots(content);
		write(&root.join(gone.relative_path()), b"{}");
		record_file(&db, &root, &gone, None).await.unwrap();
		fs::remove_file(root.join(gone.relative_path())).unwrap();

		assert_eq!(rebuild_root(&root, &db).await.unwrap(), (3, 1));
		let mut keys: Vec<_> = db
			.content_sidecars(content)
			.await
			.unwrap()
			.iter()
			.map(|row| (row.producer.clone(), row.kind.clone(), row.format.clone()))
			.collect();
		keys.sort();
		assert_eq!(
			keys,
			[
				(
					"com.spacedrive.photos".into(),
					"faces".into(),
					"json".into()
				),
				("core".into(), "proxy".into(), "mp4".into()),
				("core".into(), "transcript".into(), "json".into()),
			]
		);
		assert_eq!(rebuild_root(&root, &db).await.unwrap(), (0, 0));
	}

	#[tokio::test]
	async fn prune_keeps_content_any_source_still_holds() {
		let dir = tempfile::tempdir().unwrap();
		let a = store(dir.path(), "a").await;
		let b = Arc::new(store(dir.path(), "b").await);
		let root = dir.path().join("derived");
		let (kept_here, kept_elsewhere, gone) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
		hold(&a, kept_here).await;
		hold(&b, kept_elsewhere).await;
		for content in [kept_here, kept_elsewhere, gone] {
			let sidecar = ContentSidecar::shots(content);
			write(&root.join(sidecar.relative_path()), b"{}");
			record_file(&a, &root, &sidecar, None).await.unwrap();
		}

		assert_eq!(prune_root(&root, &a, &[b]).await.unwrap(), 1);
		for (content, kept) in [(kept_here, true), (kept_elsewhere, true), (gone, false)] {
			assert_eq!(
				root.join(sd_sidecar_path::content_directory(&content))
					.exists(),
				kept
			);
			assert_eq!(a.content_sidecars(content).await.unwrap().is_empty(), !kept);
		}
	}

	#[test]
	fn adopt_moves_a_content_whole() {
		let dir = tempfile::tempdir().unwrap();
		let content = Uuid::now_v7();
		let legacy = dir.path().join("legacy");
		let derived = dir.path().join("derived");
		let proxy = ContentSidecar::proxy(content, ProxyResolution::P720);
		write(&legacy.join(proxy.relative_path()), b"mp4");

		let contents = legacy_contents(&legacy).unwrap();
		assert_eq!(contents.len(), 1);
		let destination = derived.join(sd_sidecar_path::content_directory(&content));
		adopt_content(&contents[0].1, &destination).unwrap();
		assert_eq!(
			fs::read(derived.join(proxy.relative_path())).unwrap(),
			b"mp4"
		);
		assert!(!contents[0].1.exists());
	}

	#[test]
	fn an_interrupted_adopt_finishes_without_copies_or_partials() {
		let dir = tempfile::tempdir().unwrap();
		let content = Uuid::now_v7();
		let legacy = dir.path().join("legacy");
		let derived = dir.path().join("derived");
		let sidecars = [
			ContentSidecar::proxy(content, ProxyResolution::P720),
			ContentSidecar::transcript(content, SidecarFormat::Json),
			ContentSidecar::transcript(content, SidecarFormat::Srt),
		];
		for sidecar in &sidecars {
			write(
				&legacy.join(sidecar.relative_path()),
				sidecar.format.extension().as_bytes(),
			);
		}
		write(&sidecars[1].partial_path(&legacy), b"half");
		// The first run moved the proxy, then stopped.
		fs::create_dir_all(derived.join(sidecars[0].relative_path()).parent().unwrap()).unwrap();
		fs::rename(
			legacy.join(sidecars[0].relative_path()),
			derived.join(sidecars[0].relative_path()),
		)
		.unwrap();

		let (_, source) = legacy_contents(&legacy).unwrap().remove(0);
		let destination = derived.join(sd_sidecar_path::content_directory(&content));
		adopt_content(&source, &destination).unwrap();
		adopt_content(&source, &destination).unwrap();

		for sidecar in &sidecars {
			assert_eq!(
				fs::read(derived.join(sidecar.relative_path())).unwrap(),
				sidecar.format.extension().as_bytes()
			);
		}
		assert!(!source.exists());
		assert_eq!(scan_tree(&derived, Path::new("")).len(), 3);
		remove_empty_dirs(&legacy);
		assert_eq!(fs::read_dir(&legacy).unwrap().count(), 0);
	}

	#[test]
	fn a_content_has_one_home_across_sources() {
		let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
		// A copy already made under one source is added to, wherever the
		// next file with the same bytes lives.
		assert_eq!(choose_home(&[a], Some(b), &[]), Some(a));
		assert_eq!(choose_home(&[a, b], Some(b), &[]), Some(b));
		// The first copy goes to the source whose file produced it.
		assert_eq!(choose_home(&[], Some(b), &[c]), Some(b));
		// A writer with no file, such as an extension, uses a holder.
		assert_eq!(choose_home(&[], None, &[c]), Some(c));
		assert_eq!(choose_home(&[], None, &[]), None);
	}

	#[test]
	fn a_reader_finds_the_one_copy_under_whichever_source_holds_it() {
		let dir = tempfile::tempdir().unwrap();
		let content = Uuid::now_v7();
		let (a, b) = (dir.path().join("a/derived"), dir.path().join("b/derived"));
		let legacy = dir.path().join("legacy");
		let finder = SidecarFinder::new(vec![a.clone(), b.clone(), legacy.clone()]);
		let proxy = ContentSidecar::proxy(content, ProxyResolution::P720);
		assert_eq!(finder.find(&proxy), None);
		write(&legacy.join(proxy.relative_path()), b"old");
		assert_eq!(
			finder.find(&proxy),
			Some(legacy.join(proxy.relative_path()))
		);
		write(&b.join(proxy.relative_path()), b"mp4");
		assert_eq!(finder.find(&proxy), Some(b.join(proxy.relative_path())));
	}

	#[test]
	fn a_directory_scan_names_core_and_extension_sidecars_and_skips_partials() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path();
		let content = Uuid::now_v7();
		let proxy = ContentSidecar::proxy(content, ProxyResolution::P720);
		write(&root.join(proxy.relative_path()), b"mp4");
		write(&proxy.partial_path(root), b"half");
		let transcript = ContentSidecar::transcript(content, SidecarFormat::Json);
		write(&root.join(transcript.relative_path()), b"{}");
		let faces = root.join(sd_sidecar_path::relative_path(
			&content,
			&sd_sidecar_path::extension_kind_directory("com.spacedrive.photos", "faces"),
			"default",
			"json",
		));
		write(&faces, b"[]");
		write(&faces.with_extension("json.tmp"), b"[");

		let mut found: Vec<_> = scan_tree(root, &sd_sidecar_path::content_directory(&content))
			.iter()
			.map(|sidecar| {
				(
					sidecar.producer().to_string(),
					sidecar.kind.as_str().to_string(),
					sidecar.variant.as_str().to_string(),
					sidecar.format.extension().to_string(),
				)
			})
			.collect();
		found.sort();
		let expected: Vec<(String, String, String, String)> = [
			("com.spacedrive.photos", "faces", "default", "json"),
			("core", "proxy", "720p", "mp4"),
			("core", "transcript", "default", "json"),
		]
		.iter()
		.map(|(a, b, c, d)| (a.to_string(), b.to_string(), c.to_string(), d.to_string()))
		.collect();
		assert_eq!(found, expected);
	}
}
