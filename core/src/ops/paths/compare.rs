//! Compare what two folders hold.
//!
//! By location, the question is what a copy of a folder is missing or has
//! changed: the files one side has at a relative path the other lacks, and
//! those at the same path whose bytes differ. Both sides stream from their
//! stores in the same order, directory then name relative to their folder, so
//! a comparison is one merge of two sorted streams and a page resumes both
//! from a key.
//!
//! By content, the question is what one side holds that exists nowhere on the
//! other, whatever it is called and wherever it sits: whether everything in a
//! folder made it onto a backup. A page streams one side and asks the other
//! side's stores which of those files' content ids they hold beneath their
//! folder, a batch at a time, so no page reads the other side whole.
//!
//! Content ids come from hashing, which runs after a source is walked. A file
//! not hashed yet has no identity to match by content, so a content comparison
//! lists it nowhere, and a location comparison judges it by size and
//! modification time. Either way it is counted as unhashed, so a page says how
//! much of its answer rests on that. Hidden files take part only when asked
//! for, and bundle internals never do, as they are lensed out of search and
//! listings.

use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::reach::{stores_beneath, Reach};
use crate::context::CoreContext;
use crate::domain::{File, SdPath};
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::ops::indexing::VolumeIndex;
use sd_store::read::Start;

/// The most entries one page may ask for.
const MAX_PAGE: u32 = 5000;

/// Files a stream reads from its store at a time, and files a content
/// comparison asks the other side about at once.
const BATCH: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathCompareInput {
	pub left: SdPath,
	pub right: SdPath,
	pub by: CompareBy,
	pub show: CompareSet,
	/// Whether hidden files take part.
	#[serde(default)]
	pub include_hidden: bool,
	/// Where the previous page ended; `None` for the first page, which also
	/// counts every set.
	pub after: Option<CompareCursor>,
	/// Entries per page.
	pub limit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CompareBy {
	/// Files match when they sit at the same path relative to their folder.
	Location,
	/// Files match when they hold the same bytes, wherever they sit.
	Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CompareSet {
	/// On the left and not the right.
	OnlyLeft,
	/// On the right and not the left.
	OnlyRight,
	/// At the same place on both sides with different bytes; by location only.
	Changed,
	/// On both sides: at the same place with the same bytes by location, and
	/// the same bytes anywhere on the right by content.
	Same,
}

/// Where a page ended: its last file's directory, relative to the folder it
/// was listed from, and its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CompareCursor {
	pub directory: String,
	pub name: String,
}

/// One listed file, on the side its set names. A location comparison's changed
/// and same files have both sides, the files at the same place; a content
/// comparison lists one side only, as its bytes can sit in many places on the
/// other.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CompareEntry {
	/// Where the listed file sits relative to its side's folder, with forward
	/// slashes.
	pub path: String,
	pub left: Option<File>,
	pub right: Option<File>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CompareTotals {
	pub only_left: u32,
	pub only_right: u32,
	pub changed: u32,
	pub same: u32,
	/// Files with no content id yet: a content comparison cannot match them,
	/// and a location comparison judges them by size and modification time.
	pub unhashed_left: u32,
	pub unhashed_right: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathCompareOutput {
	pub entries: Vec<CompareEntry>,
	/// Where the next page starts; `None` after the last.
	pub next: Option<CompareCursor>,
	/// Every set's count, on the first page only, since counting reads both
	/// sides whole.
	pub totals: Option<CompareTotals>,
}

pub struct PathCompareQuery {
	input: PathCompareInput,
}

impl LibraryQuery for PathCompareQuery {
	type Input = PathCompareInput;
	type Output = PathCompareOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		if input.limit == 0 || input.limit > MAX_PAGE {
			return Err(QueryError::InvalidInput(format!(
				"limit must be between 1 and {MAX_PAGE}"
			)));
		}
		if input.by == CompareBy::Content && input.show == CompareSet::Changed {
			return Err(QueryError::InvalidInput(
				"changed files are found by location".to_string(),
			));
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let input = self.input;
		let left = stores_beneath(&context, &input.left).await;
		let right = stores_beneath(&context, &input.right).await;
		for (reached, path) in [(&left, &input.left), (&right, &input.right)] {
			if reached.is_empty() {
				return Err(QueryError::InvalidInput(format!(
					"{path} is not in a tracked source"
				)));
			}
		}

		let (left_after, right_after) = resumes(input.by, input.show, input.after.as_ref());
		let cache = context.volume_index();
		let left = Folder::open(cache, left, input.include_hidden, left_after).await?;
		let right = Folder::open(cache, right, input.include_hidden, right_after).await?;
		let page = Page {
			show: input.show,
			limit: input.limit as usize,
			count: input.after.is_none(),
			device_slug: crate::device::get_current_device_slug(),
		};
		compare(input.by, left, right, &page).await
	}
}

crate::register_library_query!(PathCompareQuery, "paths.compare");

/// Where each side resumes. By location both sides walk the same keys, so
/// both resume from the cursor. By content only the listed side streams, and
/// the other is asked about, so only the listed side resumes.
fn resumes(
	by: CompareBy,
	show: CompareSet,
	after: Option<&CompareCursor>,
) -> (Option<&CompareCursor>, Option<&CompareCursor>) {
	match (by, show) {
		(CompareBy::Location, _) => (after, after),
		(CompareBy::Content, CompareSet::OnlyRight) => (None, after),
		(CompareBy::Content, _) => (after, None),
	}
}

async fn compare(
	by: CompareBy,
	left: Folder,
	right: Folder,
	page: &Page,
) -> QueryResult<PathCompareOutput> {
	match by {
		CompareBy::Location => by_location(left, right, page).await,
		CompareBy::Content => by_content(left, right, page).await,
	}
}

/// Where a file sits relative to its side's folder: directory, then name.
type Key = (String, String);

/// Which side of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
	Left,
	Right,
}

/// What a page lists and how.
struct Page {
	show: CompareSet,
	limit: usize,
	/// Whether the page counts every set, which reads both sides whole.
	count: bool,
	device_slug: String,
}

/// Merge both sides in key order, sorting each file into its set.
async fn by_location(
	mut left: Folder,
	mut right: Folder,
	page: &Page,
) -> QueryResult<PathCompareOutput> {
	let mut totals = CompareTotals::default();
	let mut listing = Listing::new(page);
	loop {
		let order = match (left.peek().await?, right.peek().await?) {
			(None, None) => break,
			(Some(_), None) => Ordering::Less,
			(None, Some(_)) => Ordering::Greater,
			(Some(l), Some(r)) => l.cmp(&r),
		};
		let (set, l, r) = match order {
			Ordering::Less => (CompareSet::OnlyLeft, left.next().await?, None),
			Ordering::Greater => (CompareSet::OnlyRight, None, right.next().await?),
			Ordering::Equal => {
				let (l, r) = (left.next().await?, right.next().await?);
				let same = matches!((&l, &r), (Some(l), Some(r)) if same_bytes(&l.entry, &r.entry));
				let set = if same {
					CompareSet::Same
				} else {
					CompareSet::Changed
				};
				(set, l, r)
			}
		};
		for (side, file) in [(Side::Left, &l), (Side::Right, &r)] {
			if file
				.as_ref()
				.is_some_and(|file| file.entry.content_uuid.is_none())
			{
				totals.unhashed(side);
			}
		}
		totals.add(set);
		if listing.offer(set, l, r) {
			break;
		}
	}
	Ok(listing.finish(page.count.then_some(totals)))
}

/// Stream the side the page lists and ask the other which of its files' bytes
/// it holds. A counted page also streams the other side, for its counts.
async fn by_content(
	mut left: Folder,
	mut right: Folder,
	page: &Page,
) -> QueryResult<PathCompareOutput> {
	let mut totals = CompareTotals::default();
	let mut listing = Listing::new(page);
	if page.show == CompareSet::OnlyRight {
		if page.count {
			against(&mut left, &right, Side::Left, &mut totals, None).await?;
		}
		against(
			&mut right,
			&left,
			Side::Right,
			&mut totals,
			Some(&mut listing),
		)
		.await?;
	} else {
		against(
			&mut left,
			&right,
			Side::Left,
			&mut totals,
			Some(&mut listing),
		)
		.await?;
		if page.count {
			against(&mut right, &left, Side::Right, &mut totals, None).await?;
		}
	}
	Ok(listing.finish(page.count.then_some(totals)))
}

/// Stream `streamed` and ask `other`, a batch at a time, which of its files'
/// bytes it holds, counting each file's set and offering it to the listing
/// when there is one. A right file whose bytes the left holds is in no set:
/// the same bytes are counted once, from the left.
async fn against(
	streamed: &mut Folder,
	other: &Folder,
	side: Side,
	totals: &mut CompareTotals,
	mut listing: Option<&mut Listing<'_>>,
) -> QueryResult<()> {
	loop {
		let mut batch = Vec::with_capacity(BATCH);
		while batch.len() < BATCH {
			match streamed.next().await? {
				Some(file) => batch.push(file),
				None => break,
			}
		}
		if batch.is_empty() {
			return Ok(());
		}
		let contents: Vec<Uuid> = batch
			.iter()
			.filter_map(|file| file.entry.content_uuid)
			.collect();
		let held = other.holding(&contents).await?;

		for file in batch {
			let Some(content) = file.entry.content_uuid else {
				totals.unhashed(side);
				continue;
			};
			let set = match (side, held.contains(&content)) {
				(Side::Left, true) => CompareSet::Same,
				(Side::Left, false) => CompareSet::OnlyLeft,
				(Side::Right, false) => CompareSet::OnlyRight,
				(Side::Right, true) => continue,
			};
			totals.add(set);
			if let Some(listing) = listing.as_deref_mut() {
				let (l, r) = match side {
					Side::Left => (Some(file), None),
					Side::Right => (None, Some(file)),
				};
				if listing.offer(set, l, r) {
					return Ok(());
				}
			}
		}
	}
}

/// Whether two files at the same place hold the same bytes: by content id
/// where both are hashed. Where either is not, by size and modification time,
/// the check rsync makes before reading a file, since two versions of a file
/// often share a size and a copy that only looks the same is the answer a
/// comparison must not give.
fn same_bytes(left: &sd_store::FsEntry, right: &sd_store::FsEntry) -> bool {
	match (left.content_uuid, right.content_uuid) {
		(Some(left), Some(right)) => left == right,
		_ => left.size == right.size && left.mtime_ms == right.mtime_ms,
	}
}

impl CompareTotals {
	fn add(&mut self, set: CompareSet) {
		*match set {
			CompareSet::OnlyLeft => &mut self.only_left,
			CompareSet::OnlyRight => &mut self.only_right,
			CompareSet::Changed => &mut self.changed,
			CompareSet::Same => &mut self.same,
		} += 1;
	}

	fn unhashed(&mut self, side: Side) {
		*match side {
			Side::Left => &mut self.unhashed_left,
			Side::Right => &mut self.unhashed_right,
		} += 1;
	}
}

/// A page's entries, as the comparison finds them.
struct Listing<'a> {
	page: &'a Page,
	entries: Vec<CompareEntry>,
	/// The key of the last entry listed.
	last: Option<Key>,
	/// Whether an entry of the listed set turned up past a full page.
	more: bool,
}

impl<'a> Listing<'a> {
	fn new(page: &'a Page) -> Self {
		Self {
			page,
			entries: Vec::new(),
			last: None,
			more: false,
		}
	}

	/// Take a pair the comparison sorted into `set`, if that is the set the
	/// page lists. Returns whether the comparison can stop: the page is full
	/// and nothing is being counted.
	fn offer(&mut self, set: CompareSet, left: Option<Keyed>, right: Option<Keyed>) -> bool {
		if set != self.page.show {
			return false;
		}
		if self.entries.len() == self.page.limit {
			self.more = true;
			return !self.page.count;
		}
		let Some(key) = left
			.as_ref()
			.or(right.as_ref())
			.map(|file| file.key.clone())
		else {
			return false;
		};
		let slug = &self.page.device_slug;
		self.entries.push(CompareEntry {
			path: join(&key.0, &key.1),
			left: left.map(|file| file.into_file(slug)),
			right: right.map(|file| file.into_file(slug)),
		});
		self.last = Some(key);
		self.entries.len() == self.page.limit && !self.page.count
	}

	/// A counted page read both sides whole, so it knows whether more of the
	/// set followed. An uncounted one stops when full and cannot tell, so a
	/// full one always names where to resume.
	fn finish(self, totals: Option<CompareTotals>) -> PathCompareOutput {
		let full = self.entries.len() == self.page.limit;
		let next = (self.more || (full && !self.page.count))
			.then_some(self.last)
			.flatten()
			.map(|(directory, name)| CompareCursor { directory, name });
		PathCompareOutput {
			entries: self.entries,
			next,
			totals,
		}
	}
}

/// A file on one side, keyed by where it sits relative to the side's folder.
struct Keyed {
	key: Key,
	entry: sd_store::FsEntry,
	path: PathBuf,
}

impl Keyed {
	fn into_file(self, device_slug: &str) -> File {
		File::from_store_entry(
			&self.entry,
			SdPath::Physical {
				device_slug: device_slug.to_string(),
				path: self.path,
			},
		)
	}
}

/// One side's folder: the stores beneath it, merged into one stream of its
/// files in key order.
struct Folder {
	streams: Vec<Stream>,
	include_hidden: bool,
}

impl Folder {
	/// A store that cannot be read fails the comparison rather than reading as
	/// empty, which would report its files missing.
	async fn open(
		cache: &VolumeIndex,
		reached: Vec<Reach>,
		include_hidden: bool,
		after: Option<&CompareCursor>,
	) -> QueryResult<Self> {
		let mut stores = Vec::with_capacity(reached.len());
		for reach in reached {
			let Some(db) = cache.read_store(reach.source.id).await else {
				return Err(QueryError::Internal(format!(
					"no readable index for {}",
					reach.source.root.display()
				)));
			};
			stores.push((db, reach));
		}
		Ok(Self::new(stores, include_hidden, after))
	}

	fn new(
		stores: Vec<(Arc<sd_store::SourceDb>, Reach)>,
		include_hidden: bool,
		after: Option<&CompareCursor>,
	) -> Self {
		let streams = stores
			.into_iter()
			.map(|(db, reach)| Stream {
				next: after.map_or(Next::First, |cursor| {
					resume(&reach.scope, &reach.prefix, cursor)
				}),
				db,
				root: reach.source.root,
				scope: reach.scope,
				prefix: reach.prefix,
				buffer: VecDeque::new(),
			})
			.collect();
		Self {
			streams,
			include_hidden,
		}
	}

	/// Read ahead wherever a stream has run dry.
	async fn ready(&mut self) -> QueryResult<()> {
		for stream in &mut self.streams {
			stream.ready(self.include_hidden).await?;
		}
		Ok(())
	}

	/// The key of the next file in order.
	async fn peek(&mut self) -> QueryResult<Option<Key>> {
		self.ready().await?;
		Ok(self
			.streams
			.iter()
			.filter_map(|stream| stream.buffer.front())
			.map(|file| &file.key)
			.min()
			.cloned())
	}

	/// Take the next file in order.
	async fn next(&mut self) -> QueryResult<Option<Keyed>> {
		self.ready().await?;
		let first = self
			.streams
			.iter()
			.enumerate()
			.filter_map(|(index, stream)| stream.buffer.front().map(|file| (index, &file.key)))
			.min_by(|a, b| a.1.cmp(b.1))
			.map(|(index, _)| index);
		Ok(first.and_then(|index| self.streams[index].buffer.pop_front()))
	}

	/// Which of `contents` some file beneath this side's folder holds.
	async fn holding(&self, contents: &[Uuid]) -> QueryResult<HashSet<Uuid>> {
		let mut held = HashSet::new();
		for stream in &self.streams {
			held.extend(
				sd_store::read::contents_beneath(stream.db.pool(), contents, &stream.scope)
					.await
					.map_err(read_failed)?,
			);
		}
		Ok(held)
	}
}

/// One store's files beneath a side's folder, read a batch at a time.
struct Stream {
	db: Arc<sd_store::SourceDb>,
	root: PathBuf,
	/// Where the store is read from, relative to its source root.
	scope: String,
	/// Where its files sit relative to the side's folder.
	prefix: String,
	next: Next,
	buffer: VecDeque<Keyed>,
}

/// Where a stream's next read starts, in its store's own terms.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Next {
	First,
	Directories,
	After(String, String),
	Done,
}

impl Stream {
	/// Read until the stream has a file in hand or has none left. A batch can
	/// come back with nothing to hold when every row in it was lensed out.
	async fn ready(&mut self, include_hidden: bool) -> QueryResult<()> {
		while self.buffer.is_empty() && self.next != Next::Done {
			self.read(include_hidden).await?;
		}
		Ok(())
	}

	async fn read(&mut self, include_hidden: bool) -> QueryResult<()> {
		let next = std::mem::replace(&mut self.next, Next::Done);
		let start = match &next {
			Next::First => Start::First,
			Next::Directories => Start::Directories,
			Next::After(directory, name) => Start::After { directory, name },
			Next::Done => return Ok(()),
		};
		let batch = sd_store::read::files_beneath(
			self.db.pool(),
			&self.scope,
			start,
			None,
			include_hidden,
			BATCH,
		)
		.await
		.map_err(read_failed)?;

		if batch.len() == BATCH {
			if let Some(last) = batch.last() {
				self.next = Next::After(last.directory().to_string(), last.name.clone());
			}
		}
		for entry in batch {
			let path = self.root.join(&entry.relative_path);
			// Bundle internals surface through the source that describes them,
			// as they do in search.
			if crate::ops::indexing::lens::is_bundle_internal(&path) {
				continue;
			}
			let directory = join(&self.prefix, beneath(&self.scope, entry.directory()));
			let key = (directory, entry.name.clone());
			self.buffer.push_back(Keyed { key, entry, path });
		}
		Ok(())
	}
}

/// Where a store resumes after `cursor`, a place relative to the side's
/// folder, for a store read from `scope` whose files sit at `prefix`.
///
/// The store holding the folder has every place in its terms. A store nested
/// beneath the folder holds only its own subtree, so a cursor before it starts
/// it from the top, one past it finishes it, and one that falls between its
/// root-level files and its directories, like a sibling "m-2" beside a source
/// at "m", resumes it at its first directory.
fn resume(scope: &str, prefix: &str, cursor: &CompareCursor) -> Next {
	let (directory, name) = (cursor.directory.as_str(), cursor.name.clone());
	if prefix.is_empty() {
		return Next::After(join(scope, directory), name);
	}
	if directory == prefix {
		return Next::After(String::new(), name);
	}
	if let Some(beneath) = directory
		.strip_prefix(prefix)
		.and_then(|rest| rest.strip_prefix('/'))
	{
		return Next::After(beneath.to_string(), name);
	}
	if directory < prefix {
		Next::First
	} else if directory < format!("{prefix}/").as_str() {
		Next::Directories
	} else {
		Next::Done
	}
}

/// `directory` relative to `scope`, both relative to one source root.
fn beneath<'a>(scope: &str, directory: &'a str) -> &'a str {
	if scope.is_empty() {
		return directory;
	}
	if directory == scope {
		return "";
	}
	directory
		.strip_prefix(scope)
		.and_then(|rest| rest.strip_prefix('/'))
		.unwrap_or(directory)
}

fn join(base: &str, rest: &str) -> String {
	match (base.is_empty(), rest.is_empty()) {
		(true, _) => rest.to_string(),
		(false, true) => base.to_string(),
		(false, false) => format!("{base}/{rest}"),
	}
}

fn read_failed(error: sd_store::Error) -> QueryError {
	QueryError::Internal(format!("compare read failed: {error}"))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::paths::reach::source;
	use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
	use sd_store::record::ContentIdentity;

	/// When every fixture file was last modified, unless it says otherwise.
	const T: i64 = 1_700_000_000_000;

	/// Folder L on a volume at /vol, holding a source nested at L/m, against
	/// folder R beside it on the same volume. L-2 shares L's prefix and holds
	/// bytes that exist in R only, so reading past L's edge would show.
	struct Fixture {
		_dir: tempfile::TempDir,
		outer: Arc<sd_store::SourceDb>,
		nested: Arc<sd_store::SourceDb>,
	}

	impl Fixture {
		async fn new() -> Self {
			let dir = tempfile::tempdir().expect("tempdir");
			let outer = store(
				&dir,
				"outer",
				&[
					("L/a.txt", 42, T, Some("a")),
					("L/b.txt", 42, T, Some("b")),
					("L/moved.txt", 42, T, Some("moved")),
					("L/raw.bin", 42, T, None),
					("L/m-2/f.txt", 42, T, Some("a")),
					("L/sub/c.txt", 42, T, Some("c")),
					("L/sub/edited.txt", 42, T, Some("e1")),
					("L/sub/log.txt", 42, T, None),
					("L-2/stray.txt", 42, T, Some("x")),
					("R/a.txt", 42, T, Some("a")),
					("R/b.txt", 42, T, Some("b2")),
					("R/raw.bin", 42, T, None),
					("R/elsewhere/moved.txt", 42, T, Some("moved")),
					("R/sub/edited.txt", 42, T, Some("e2")),
					("R/sub/extra.txt", 42, T, Some("x")),
					("R/sub/log.txt", 42, T + 1000, None),
				],
			)
			.await;
			let nested = store(
				&dir,
				"nested",
				&[
					("n.txt", 42, T, Some("n")),
					("deep/d.txt", 42, T, Some("d")),
				],
			)
			.await;
			Self {
				_dir: dir,
				outer,
				nested,
			}
		}

		fn left(&self, after: Option<&CompareCursor>) -> Folder {
			Folder::new(
				vec![
					(self.outer.clone(), reach("/vol", "L", "")),
					(self.nested.clone(), reach("/vol/L/m", "", "m")),
				],
				false,
				after,
			)
		}

		fn right(&self, after: Option<&CompareCursor>) -> Folder {
			Folder::new(
				vec![(self.outer.clone(), reach("/vol", "R", ""))],
				false,
				after,
			)
		}
	}

	fn reach(root: &str, scope: &str, prefix: &str) -> Reach {
		Reach {
			source: source(root),
			scope: scope.to_string(),
			prefix: prefix.to_string(),
		}
	}

	/// A store holding `files` and the directories above them, each file with
	/// its size, modification time and the sampled hash its content id
	/// derives from.
	async fn store(
		dir: &tempfile::TempDir,
		name: &str,
		files: &[(&str, i64, i64, Option<&str>)],
	) -> Arc<sd_store::SourceDb> {
		let manager = sd_store::SourceManager::new(dir.path().to_path_buf());
		manager
			.create(name, &sd_store::filesystem_schema())
			.await
			.expect("create");
		let db = manager.open(name).await.expect("open");
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

		// A walk writes every directory before its children, so each write
		// knows the parent it hangs from.
		let mut writes = Vec::new();
		let mut directories = HashSet::new();
		for (path, size, mtime, _) in files {
			for (end, _) in path.match_indices('/') {
				let directory = &path[..end];
				if directories.insert(directory) {
					writes.push(write(&mut ledger, directory, FileKind::Directory, 0, T));
				}
			}
			writes.push(write(&mut ledger, path, FileKind::File, *size, *mtime));
		}
		db.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");

		for (path, size, _, hash) in files {
			let Some(hash) = hash else {
				continue;
			};
			let record = sd_store::read::entry_by_path(db.pool(), path)
				.await
				.expect("lookup")
				.expect("written")
				.uuid;
			db.set_content_identity(
				record,
				&ContentIdentity {
					sampled_hash: Some(hash.to_string()),
					size: Some(*size),
					..Default::default()
				},
			)
			.await
			.expect("identity");
		}
		Arc::new(db)
	}

	fn write(ledger: &mut Ledger, path: &str, kind: FileKind, size: i64, mtime: i64) -> FileWrite {
		let observation = Observation {
			external_id: path.to_string(),
			kind,
			name: path.rsplit('/').next().unwrap_or(path).to_string(),
			size,
			mtime,
			created: None,
			accessed: None,
			inode: None,
			mode: Some(0o644),
			uid: None,
			gid: None,
			link_target: None,
			extension: path.rsplit_once('.').map(|(_, e)| e.to_string()),
			is_hidden: false,
			identity: None,
		};
		let resolution = ledger.resolve(&observation);
		let parent_uuid = path
			.rsplit_once('/')
			.and_then(|(parent, _)| ledger.uuid_of(parent));
		FileWrite {
			resolution,
			parent_uuid,
			observation,
		}
	}

	async fn page(
		fixture: &Fixture,
		by: CompareBy,
		show: CompareSet,
		limit: usize,
		after: Option<&CompareCursor>,
	) -> PathCompareOutput {
		let (left_after, right_after) = resumes(by, show, after);
		let page = Page {
			show,
			limit,
			count: after.is_none(),
			device_slug: "dev".to_string(),
		};
		compare(
			by,
			fixture.left(left_after),
			fixture.right(right_after),
			&page,
		)
		.await
		.expect("compare")
	}

	/// Each listed entry as the paths it names beneath the volume, the left's
	/// first where it has both.
	fn listed(output: &PathCompareOutput) -> Vec<String> {
		let named = |file: &Option<File>| {
			file.as_ref().map(|file| match &file.sd_path {
				SdPath::Physical { path, .. } => path
					.strip_prefix("/vol")
					.expect("beneath the volume")
					.display()
					.to_string(),
				other => panic!("{other:?} is not a physical path"),
			})
		};
		output
			.entries
			.iter()
			.map(|entry| match (named(&entry.left), named(&entry.right)) {
				(Some(left), Some(right)) => format!("{left} {right}"),
				(Some(side), None) | (None, Some(side)) => side,
				(None, None) => panic!("an entry names no file"),
			})
			.collect()
	}

	/// Each listed entry's path relative to its side's folder.
	fn paths(output: &PathCompareOutput) -> Vec<&str> {
		output
			.entries
			.iter()
			.map(|entry| entry.path.as_str())
			.collect()
	}

	/// Every page of a set, following each page's cursor to the end.
	async fn every_page(
		fixture: &Fixture,
		by: CompareBy,
		show: CompareSet,
		limit: usize,
	) -> Vec<String> {
		let mut all = Vec::new();
		let mut after = None;
		for _ in 0..64 {
			let output = page(fixture, by, show, limit, after.as_ref()).await;
			assert_eq!(
				output.totals.is_some(),
				after.is_none(),
				"the first page counts"
			);
			assert!(output.entries.len() <= limit);
			all.extend(listed(&output));
			match output.next {
				Some(next) => after = Some(next),
				None => return all,
			}
		}
		panic!("{by:?} {show:?} by {limit} never reached a last page");
	}

	/// A cursor lands in the store holding the folder as a place in its own
	/// terms, and in a nested store by where the nested source sits: before
	/// it, in it, between its root-level files and its directories, or past
	/// it.
	#[test]
	fn a_cursor_resumes_each_store_in_its_own_terms() {
		let at = |directory: &str, name: &str| CompareCursor {
			directory: directory.to_string(),
			name: name.to_string(),
		};
		let after =
			|directory: &str, name: &str| Next::After(directory.to_string(), name.to_string());

		assert_eq!(
			resume("L", "", &at("sub", "c.txt")),
			after("L/sub", "c.txt")
		);
		assert_eq!(resume("L", "", &at("", "a.txt")), after("L", "a.txt"));
		assert_eq!(resume("", "", &at("", "a.txt")), after("", "a.txt"));

		assert_eq!(resume("", "m", &at("", "z.txt")), Next::First);
		assert_eq!(resume("", "m", &at("l/z", "z.txt")), Next::First);
		assert_eq!(resume("", "m", &at("m", "n.txt")), after("", "n.txt"));
		assert_eq!(
			resume("", "m", &at("m/deep", "d.txt")),
			after("deep", "d.txt")
		);
		assert_eq!(resume("", "m", &at("m-2", "f.txt")), Next::Directories);
		assert_eq!(resume("", "m", &at("m0", "a.txt")), Next::Done);
		assert_eq!(resume("", "m", &at("sub", "c.txt")), Next::Done);
	}

	/// By location a file's set is decided at its relative path: missing on
	/// one side, or present on both with the same or different bytes. Bytes
	/// compare by content id where both sides are hashed, and by size and
	/// modification time where either is not.
	#[tokio::test]
	async fn by_location_sorts_each_file_into_its_set() {
		let fixture = Fixture::new().await;
		let first = |show| page(&fixture, CompareBy::Location, show, 100, None);

		let only_left = first(CompareSet::OnlyLeft).await;
		assert_eq!(
			listed(&only_left),
			[
				"L/moved.txt",
				"L/m/n.txt",
				"L/m-2/f.txt",
				"L/m/deep/d.txt",
				"L/sub/c.txt",
			]
		);
		assert_eq!(
			paths(&only_left),
			[
				"moved.txt",
				"m/n.txt",
				"m-2/f.txt",
				"m/deep/d.txt",
				"sub/c.txt"
			]
		);
		assert_eq!(only_left.next, None);
		assert_eq!(
			only_left.totals,
			Some(CompareTotals {
				only_left: 5,
				only_right: 2,
				changed: 3,
				same: 2,
				unhashed_left: 2,
				unhashed_right: 2,
			})
		);

		assert_eq!(
			listed(&first(CompareSet::OnlyRight).await),
			["R/elsewhere/moved.txt", "R/sub/extra.txt"]
		);
		assert_eq!(
			listed(&first(CompareSet::Changed).await),
			[
				"L/b.txt R/b.txt",
				"L/sub/edited.txt R/sub/edited.txt",
				"L/sub/log.txt R/sub/log.txt",
			]
		);
		assert_eq!(
			listed(&first(CompareSet::Same).await),
			["L/a.txt R/a.txt", "L/raw.bin R/raw.bin"]
		);
	}

	/// By content a file's set is decided by whether its bytes sit anywhere
	/// beneath the other folder. A right file whose bytes the left holds is
	/// counted once, from the left, and an unhashed file is counted apart.
	#[tokio::test]
	async fn by_content_finds_bytes_wherever_they_sit() {
		let fixture = Fixture::new().await;
		let first = |show| page(&fixture, CompareBy::Content, show, 100, None);

		let only_left = first(CompareSet::OnlyLeft).await;
		assert_eq!(
			listed(&only_left),
			[
				"L/b.txt",
				"L/m/n.txt",
				"L/m/deep/d.txt",
				"L/sub/c.txt",
				"L/sub/edited.txt",
			]
		);
		assert_eq!(
			only_left.totals,
			Some(CompareTotals {
				only_left: 5,
				only_right: 3,
				changed: 0,
				same: 3,
				unhashed_left: 2,
				unhashed_right: 2,
			})
		);

		let only_right = first(CompareSet::OnlyRight).await;
		assert_eq!(
			listed(&only_right),
			["R/b.txt", "R/sub/edited.txt", "R/sub/extra.txt"]
		);
		assert_eq!(
			paths(&only_right),
			["b.txt", "sub/edited.txt", "sub/extra.txt"]
		);
		assert_eq!(
			listed(&first(CompareSet::Same).await),
			["L/a.txt", "L/moved.txt", "L/m-2/f.txt"]
		);
	}

	/// Following cursors page by page lists a set exactly as one page does,
	/// at page sizes that end pages inside the nested store, at its edge and
	/// between its root-level files and its directories.
	#[tokio::test]
	async fn pages_resume_where_the_last_ended() {
		let fixture = Fixture::new().await;
		let sets = [
			(CompareBy::Location, CompareSet::OnlyLeft),
			(CompareBy::Location, CompareSet::OnlyRight),
			(CompareBy::Location, CompareSet::Changed),
			(CompareBy::Location, CompareSet::Same),
			(CompareBy::Content, CompareSet::OnlyLeft),
			(CompareBy::Content, CompareSet::OnlyRight),
			(CompareBy::Content, CompareSet::Same),
		];
		for (by, show) in sets {
			let whole = listed(&page(&fixture, by, show, 100, None).await);
			for limit in 1..=3 {
				assert_eq!(
					every_page(&fixture, by, show, limit).await,
					whole,
					"{by:?} {show:?} by {limit}"
				);
			}
		}
	}

	#[test]
	fn a_page_asks_only_what_a_comparison_answers() {
		let input = |by, show, limit| PathCompareInput {
			left: SdPath::local("/a"),
			right: SdPath::local("/b"),
			by,
			show,
			include_hidden: false,
			after: None,
			limit,
		};
		let invalid = |input| {
			matches!(
				PathCompareQuery::from_input(input),
				Err(QueryError::InvalidInput(_))
			)
		};
		assert!(
			PathCompareQuery::from_input(input(CompareBy::Location, CompareSet::Changed, 100))
				.is_ok()
		);
		assert!(invalid(input(CompareBy::Content, CompareSet::Changed, 100)));
		assert!(invalid(input(CompareBy::Location, CompareSet::Same, 0)));
		assert!(invalid(input(
			CompareBy::Location,
			CompareSet::Same,
			MAX_PAGE + 1
		)));
	}
}
