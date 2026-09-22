//! Compare what two folders, A and B, hold.
//!
//! By path, the question is how a copy of a folder differs from it: the files
//! one side has at a relative path the other lacks, and those at the same path
//! whose bytes differ. Both sides stream from their stores in the same order,
//! directory then name relative to their folder, so a comparison is one merge
//! of two sorted streams and a page resumes both from a key.
//!
//! By content, the question is what one side holds that exists nowhere in the
//! other, whatever it is called and wherever it sits: whether everything in a
//! folder made it onto a backup. A page streams one side and asks the other
//! side's stores which of those files' contents they hold beneath their
//! folder, a batch at a time, so no page reads the other side whole.
//!
//! Files match on the hash ladder. Every store keys a content by its sampled
//! hash, so that is what two files are matched by, whichever rung each store
//! has reached; where both have read the bytes in full, the integrity hashes
//! decide. A sampled match is a candidate, which is what a comparison lists.
//! Removing a copy on the strength of one is the job of an operation that
//! reads the bytes first. Hashing runs after a source is walked, so a file not
//! hashed yet has nothing to match by: a content comparison lists it nowhere,
//! and a path comparison judges it by size and modification time. Either way
//! it is counted as unhashed, so a page says how much of its answer rests on
//! that. Hidden files take part only when asked for, and bundle internals
//! never do, as they are lensed out of search and listings.
//!
//! The query pages a comparison. [`Matcher`] is the comparison itself, which
//! an operation over one set of files, like deleting from A what B holds,
//! drains in the same order and resumes from the same cursor.

use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::reach::{stores_beneath, Reach};
use crate::context::CoreContext;
use crate::domain::{File, SdPath};
use crate::infra::query::{LibraryQuery, QueryError, QueryResult};
use crate::ops::indexing::VolumeIndex;
use sd_store::read::Start;
use sd_store::FsEntry;

/// The most entries one page may ask for.
pub const MAX_PAGE: u32 = 5000;

/// Files a stream reads from its store at a time, and files a content
/// comparison asks the other side about at once.
const BATCH: usize = 1000;

/// What a comparison is over: two folders, how their files match, and the set
/// in question. An operation names its targets with one too, so it stands
/// apart from how a page of it is read.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Comparison {
	pub a: SdPath,
	pub b: SdPath,
	pub by: CompareBy,
	pub show: CompareSet,
	/// Whether hidden files take part.
	#[serde(default)]
	pub include_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathCompareInput {
	#[serde(flatten)]
	pub comparison: Comparison,
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
	Path,
	/// Files match when they hold the same bytes, wherever they sit.
	Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CompareSet {
	/// In A and not in B.
	OnlyA,
	/// In B and not in A.
	OnlyB,
	/// The same file in both: at the same path with the same bytes by path,
	/// and the same bytes anywhere in B by content.
	Both,
	/// At the same path in both with different bytes; by path only.
	Different,
}

/// Where a page ended: its last file's directory, relative to the folder it
/// was listed from, and its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CompareCursor {
	pub directory: String,
	pub name: String,
}

/// One listed file, in the folder its set names. A path comparison's both and
/// different files have both sides, the files at the same path; a content
/// comparison lists one side only, as its bytes can sit in many places in the
/// other.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CompareEntry {
	/// Where the listed file sits relative to its side's folder, with forward
	/// slashes.
	pub path: String,
	pub a: Option<File>,
	pub b: Option<File>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CompareTotals {
	pub only_a: u32,
	pub only_b: u32,
	pub both: u32,
	pub different: u32,
	/// Files not hashed yet: a content comparison cannot match them, and a
	/// path comparison judges them by size and modification time.
	pub unhashed_a: u32,
	pub unhashed_b: u32,
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
		if input.comparison.by == CompareBy::Content
			&& input.comparison.show == CompareSet::Different
		{
			return Err(QueryError::InvalidInput(
				"different files are found by path".to_string(),
			));
		}
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let PathCompareInput {
			comparison,
			after,
			limit,
		} = self.input;
		let (a, b) = open_folders(&context, &comparison, after.as_ref()).await?;
		let page = Page {
			show: comparison.show,
			limit: limit as usize,
			count: after.is_none(),
			device_slug: crate::device::get_current_device_slug(),
		};
		compare(comparison.by, a, b, &page).await
	}
}

crate::register_library_query!(PathCompareQuery, "paths.compare");

/// Both folders of a comparison, open to read on from `after`. A folder
/// outside every tracked source has no store to read.
pub(crate) async fn open_folders(
	context: &CoreContext,
	comparison: &Comparison,
	after: Option<&CompareCursor>,
) -> QueryResult<(Folder, Folder)> {
	let a = stores_beneath(context, &comparison.a).await;
	let b = stores_beneath(context, &comparison.b).await;
	for (reached, path) in [(&a, &comparison.a), (&b, &comparison.b)] {
		if reached.is_empty() {
			return Err(QueryError::InvalidInput(format!(
				"{path} is not in a tracked source"
			)));
		}
	}
	let (a_after, b_after) = resumes(comparison.by, comparison.show, after);
	let cache = context.volume_index();
	Ok((
		Folder::open(cache, a, comparison.include_hidden, a_after).await?,
		Folder::open(cache, b, comparison.include_hidden, b_after).await?,
	))
}

/// Where each side resumes. By path both sides walk the same keys, so both
/// resume from the cursor. By content only the listed side streams, and the
/// other is asked about, so only the listed side resumes.
fn resumes(
	by: CompareBy,
	show: CompareSet,
	after: Option<&CompareCursor>,
) -> (Option<&CompareCursor>, Option<&CompareCursor>) {
	match (by, show) {
		(CompareBy::Path, _) => (after, after),
		(CompareBy::Content, CompareSet::OnlyB) => (None, after),
		(CompareBy::Content, _) => (after, None),
	}
}

/// One page: the listed set's files as the matcher sorts them, and every
/// set's count when the page counts. By content only the listed side's sets
/// come out of its matcher, so a counted page streams the other side too.
async fn compare(
	by: CompareBy,
	mut a: Folder,
	mut b: Folder,
	page: &Page,
) -> QueryResult<PathCompareOutput> {
	let mut listing = Listing::new(page);
	let totals = match by {
		CompareBy::Path => {
			let mut matcher = Matcher::by_path(&mut a, &mut b);
			while let Some(sorted) = matcher.next().await? {
				if listing.offer(sorted) {
					break;
				}
			}
			matcher.totals
		}
		CompareBy::Content => {
			let listed = if page.show == CompareSet::OnlyB {
				Side::B
			} else {
				Side::A
			};
			let mut matcher = match listed {
				Side::A => Matcher::by_content(&mut a, &b, Side::A),
				Side::B => Matcher::by_content(&mut b, &a, Side::B),
			};
			while let Some(sorted) = matcher.next().await? {
				if listing.offer(sorted) {
					break;
				}
			}
			let mut totals = matcher.totals;
			if page.count {
				let mut other = match listed {
					Side::A => Matcher::by_content(&mut b, &a, Side::B),
					Side::B => Matcher::by_content(&mut a, &b, Side::A),
				};
				while other.next().await?.is_some() {}
				totals.merge(other.totals);
			}
			totals
		}
	};
	Ok(listing.finish(page.count.then_some(totals)))
}

/// Where a file sits relative to its side's folder: directory, then name.
pub(crate) type Key = (String, String);

/// Which side of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
	A,
	B,
}

/// What a page lists and how.
struct Page {
	show: CompareSet,
	limit: usize,
	/// Whether the page counts every set, which reads both sides whole.
	count: bool,
	device_slug: String,
}

/// One file a comparison sorted, and the file at its path on the other side
/// when the comparison is by path.
pub(crate) struct Sorted {
	pub(crate) set: CompareSet,
	pub(crate) a: Option<Keyed>,
	pub(crate) b: Option<Keyed>,
}

impl Sorted {
	/// Where the sorted file sits, which a cursor names.
	pub(crate) fn key(&self) -> Option<&Key> {
		self.a.as_ref().or(self.b.as_ref()).map(|file| &file.key)
	}
}

/// A comparison: two folders' files sorted into sets, in key order, each
/// counted as it is sorted.
///
/// By path both folders stream and merge. By content one side streams and
/// the other is asked, a batch at a time, which of the batch's contents it
/// holds, so only the streamed side's sets come out, and a file in B whose
/// bytes A holds is in no set: the same bytes are counted once, from A.
pub(crate) struct Matcher<'f> {
	matching: Matching<'f>,
	pub(crate) totals: CompareTotals,
}

enum Matching<'f> {
	Path {
		a: &'f mut Folder,
		b: &'f mut Folder,
	},
	Content {
		streamed: &'f mut Folder,
		other: &'f Folder,
		side: Side,
		sorted: VecDeque<Sorted>,
	},
}

impl<'f> Matcher<'f> {
	pub(crate) fn by_path(a: &'f mut Folder, b: &'f mut Folder) -> Self {
		Self {
			matching: Matching::Path { a, b },
			totals: CompareTotals::default(),
		}
	}

	/// Streams `streamed`, the folder of `side`, against `other`.
	pub(crate) fn by_content(streamed: &'f mut Folder, other: &'f Folder, side: Side) -> Self {
		Self {
			matching: Matching::Content {
				streamed,
				other,
				side,
				sorted: VecDeque::new(),
			},
			totals: CompareTotals::default(),
		}
	}

	/// One file in the other folder for each of `contents`, by sampled hash,
	/// that it holds: B by path, and by content the folder being asked.
	pub(crate) async fn holders(&self, contents: &[String]) -> QueryResult<Vec<Keyed>> {
		match &self.matching {
			Matching::Path { b, .. } => b.holders(contents).await,
			Matching::Content { other, .. } => other.holders(contents).await,
		}
	}

	/// The next file in key order, sorted.
	pub(crate) async fn next(&mut self) -> QueryResult<Option<Sorted>> {
		match &mut self.matching {
			Matching::Path { a, b } => merge(a, b, &mut self.totals).await,
			Matching::Content {
				streamed,
				other,
				side,
				sorted,
			} => {
				while sorted.is_empty() {
					if !ask(streamed, other, *side, sorted, &mut self.totals).await? {
						return Ok(None);
					}
				}
				Ok(sorted.pop_front())
			}
		}
	}
}

/// One step of the merge of both sides in key order.
async fn merge(
	a: &mut Folder,
	b: &mut Folder,
	totals: &mut CompareTotals,
) -> QueryResult<Option<Sorted>> {
	let order = match (a.peek().await?, b.peek().await?) {
		(None, None) => return Ok(None),
		(Some(_), None) => Ordering::Less,
		(None, Some(_)) => Ordering::Greater,
		(Some(key_a), Some(key_b)) => key_a.cmp(&key_b),
	};
	let (set, file_a, file_b) = match order {
		Ordering::Less => (CompareSet::OnlyA, a.next().await?, None),
		Ordering::Greater => (CompareSet::OnlyB, None, b.next().await?),
		Ordering::Equal => {
			let (file_a, file_b) = (a.next().await?, b.next().await?);
			let same =
				matches!((&file_a, &file_b), (Some(x), Some(y)) if same_bytes(&x.entry, &y.entry));
			let set = if same {
				CompareSet::Both
			} else {
				CompareSet::Different
			};
			(set, file_a, file_b)
		}
	};
	for (side, file) in [(Side::A, &file_a), (Side::B, &file_b)] {
		if file.as_ref().is_some_and(|file| !hashed(&file.entry)) {
			totals.unhashed(side);
		}
	}
	totals.add(set);
	Ok(Some(Sorted {
		set,
		a: file_a,
		b: file_b,
	}))
}

/// Read a batch of `side`'s files from `streamed` and ask `other` which of
/// their contents it holds, sorting each. Whether anything was read.
async fn ask(
	streamed: &mut Folder,
	other: &Folder,
	side: Side,
	sorted: &mut VecDeque<Sorted>,
	totals: &mut CompareTotals,
) -> QueryResult<bool> {
	let mut batch = Vec::with_capacity(BATCH);
	while batch.len() < BATCH {
		match streamed.next().await? {
			Some(file) => batch.push(file),
			None => break,
		}
	}
	if batch.is_empty() {
		return Ok(false);
	}
	let contents: Vec<String> = batch
		.iter()
		.filter_map(|file| file.entry.sampled_hash.clone())
		.collect();
	let held = other.holding(&contents).await?;

	for file in batch {
		let set = match (side, holds(&held, &file.entry)) {
			(_, None) => {
				totals.unhashed(side);
				continue;
			}
			(Side::A, Some(true)) => CompareSet::Both,
			(Side::A, Some(false)) => CompareSet::OnlyA,
			(Side::B, Some(false)) => CompareSet::OnlyB,
			(Side::B, Some(true)) => continue,
		};
		totals.add(set);
		let (a, b) = match side {
			Side::A => (Some(file), None),
			Side::B => (None, Some(file)),
		};
		sorted.push_back(Sorted { set, a, b });
	}
	Ok(true)
}

/// Whether two files at the same path hold the same bytes: by integrity hash
/// where both have been read in full, else by sampled hash where both have
/// one. Where either has neither, by size and modification time, the check
/// rsync makes before reading a file, since two versions of a file often
/// share a size and a copy that only looks the same is the answer a
/// comparison must not give.
fn same_bytes(a: &FsEntry, b: &FsEntry) -> bool {
	let integrity = (a.integrity_hash.as_deref(), b.integrity_hash.as_deref());
	let sampled = (a.sampled_hash.as_deref(), b.sampled_hash.as_deref());
	match (integrity, sampled) {
		((Some(x), Some(y)), _) => x == y,
		(_, (Some(x), Some(y))) => x == y,
		_ => a.size == b.size && a.mtime_ms == b.mtime_ms,
	}
}

/// Whether the other side holds a file's bytes, from what it answered about
/// the file's batch: `None` for a file with nothing to ask by. Where both
/// sides have read the bytes in full the integrity hashes decide; otherwise
/// the sampled match stands.
fn holds(held: &HashMap<String, Option<String>>, entry: &FsEntry) -> Option<bool> {
	let sampled = entry.sampled_hash.as_deref()?;
	Some(match (held.get(sampled), entry.integrity_hash.as_deref()) {
		(None, _) => false,
		(Some(Some(theirs)), Some(ours)) => theirs == ours,
		(Some(_), None) | (Some(None), _) => true,
	})
}

/// Whether a file has a hash to be matched by.
fn hashed(entry: &FsEntry) -> bool {
	entry.sampled_hash.is_some()
}

impl CompareTotals {
	/// How many files a set holds.
	pub fn count(&self, set: CompareSet) -> u32 {
		match set {
			CompareSet::OnlyA => self.only_a,
			CompareSet::OnlyB => self.only_b,
			CompareSet::Both => self.both,
			CompareSet::Different => self.different,
		}
	}

	fn add(&mut self, set: CompareSet) {
		*match set {
			CompareSet::OnlyA => &mut self.only_a,
			CompareSet::OnlyB => &mut self.only_b,
			CompareSet::Both => &mut self.both,
			CompareSet::Different => &mut self.different,
		} += 1;
	}

	fn unhashed(&mut self, side: Side) {
		*match side {
			Side::A => &mut self.unhashed_a,
			Side::B => &mut self.unhashed_b,
		} += 1;
	}

	/// Take in the other side's counts from a content comparison, whose sets
	/// are disjoint from this side's.
	fn merge(&mut self, other: CompareTotals) {
		self.only_a += other.only_a;
		self.only_b += other.only_b;
		self.both += other.both;
		self.different += other.different;
		self.unhashed_a += other.unhashed_a;
		self.unhashed_b += other.unhashed_b;
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

	/// Take a sorted file, if its set is the one the page lists. Returns
	/// whether the comparison can stop: the page is full and nothing is being
	/// counted.
	fn offer(&mut self, sorted: Sorted) -> bool {
		if sorted.set != self.page.show {
			return false;
		}
		if self.entries.len() == self.page.limit {
			self.more = true;
			return !self.page.count;
		}
		let Some(key) = sorted.key().cloned() else {
			return false;
		};
		let slug = &self.page.device_slug;
		self.entries.push(CompareEntry {
			path: join(&key.0, &key.1),
			a: sorted.a.map(|file| file.into_file(slug)),
			b: sorted.b.map(|file| file.into_file(slug)),
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
pub(crate) struct Keyed {
	pub(crate) key: Key,
	pub(crate) entry: FsEntry,
	/// Where the file is on disk.
	pub(crate) path: PathBuf,
	/// The root of the source whose store holds it.
	pub(crate) root: Arc<PathBuf>,
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
pub(crate) struct Folder {
	streams: Vec<Stream>,
	include_hidden: bool,
}

impl Folder {
	/// A store that cannot be read fails the comparison rather than reading as
	/// empty, which would report its files missing.
	pub(crate) async fn open(
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
				root: Arc::new(reach.source.root),
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

	/// Which of `contents`, by sampled hash, some file beneath this side's
	/// folder holds, with the content's integrity hash where the store
	/// holding it has read the bytes in full.
	async fn holding(&self, contents: &[String]) -> QueryResult<HashMap<String, Option<String>>> {
		let mut held: HashMap<String, Option<String>> = HashMap::new();
		for stream in &self.streams {
			let found = sd_store::read::contents_beneath(stream.db.pool(), contents, &stream.scope)
				.await
				.map_err(read_failed)?;
			// Of the stores holding a content, one that has read it in full
			// answers for it.
			for (sampled, integrity) in found {
				let known = held.entry(sampled).or_insert(None);
				if known.is_none() {
					*known = integrity;
				}
			}
		}
		Ok(held)
	}

	/// One file beneath this side's folder for each of `contents`, by sampled
	/// hash, that some file there holds: the file an operation reads in full
	/// to settle the content.
	pub(crate) async fn holders(&self, contents: &[String]) -> QueryResult<Vec<Keyed>> {
		let mut holders = Vec::new();
		for stream in &self.streams {
			let found = sd_store::read::holders_beneath(stream.db.pool(), contents, &stream.scope)
				.await
				.map_err(read_failed)?;
			holders.extend(found.into_iter().filter_map(|entry| stream.keyed(entry)));
		}
		Ok(holders)
	}
}

/// One store's files beneath a side's folder, read a batch at a time.
struct Stream {
	db: Arc<sd_store::SourceDb>,
	root: Arc<PathBuf>,
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
			if let Some(keyed) = self.keyed(entry) {
				self.buffer.push_back(keyed);
			}
		}
		Ok(())
	}

	/// A store row as a file beneath the side's folder; none for a bundle
	/// internal, which surfaces through the source describing it, as in
	/// search.
	fn keyed(&self, entry: FsEntry) -> Option<Keyed> {
		let path = self.root.join(&entry.relative_path);
		if crate::ops::indexing::lens::is_bundle_internal(&path) {
			return None;
		}
		let directory = join(&self.prefix, beneath(&self.scope, entry.directory()));
		Some(Keyed {
			key: (directory, entry.name.clone()),
			entry,
			path,
			root: self.root.clone(),
		})
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
	use std::collections::HashSet;

	/// When every fixture file was last modified, unless it says otherwise.
	const T: i64 = 1_700_000_000_000;

	/// Folder A on a volume at /vol, holding a source nested at A/m, against
	/// folder B beside it on the same volume. A-2 shares A's prefix and holds
	/// bytes that exist in B only, so reading past A's edge would show.
	/// m/verified.txt has been read in full in A and not in B, and the sides
	/// of m/twin.txt share a sampled hash with different bytes, which only
	/// their integrity hashes tell apart. Each of those pairs spans two
	/// stores, as a store keys one content per sampled hash.
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
					("A/a.txt", 42, T, Some("a"), None),
					("A/b.txt", 42, T, Some("b"), None),
					("A/moved.txt", 42, T, Some("moved"), None),
					("A/raw.bin", 42, T, None, None),
					("A/m-2/f.txt", 42, T, Some("a"), None),
					("A/sub/c.txt", 42, T, Some("c"), None),
					("A/sub/edited.txt", 42, T, Some("e1"), None),
					("A/sub/log.txt", 42, T, None, None),
					("A-2/stray.txt", 42, T, Some("x"), None),
					("B/a.txt", 42, T, Some("a"), None),
					("B/b.txt", 42, T, Some("b2"), None),
					("B/raw.bin", 42, T, None, None),
					("B/elsewhere/moved.txt", 42, T, Some("moved"), None),
					("B/m/twin.txt", 42, T, Some("t"), Some("T2")),
					("B/m/verified.txt", 42, T, Some("v"), None),
					("B/sub/edited.txt", 42, T, Some("e2"), None),
					("B/sub/extra.txt", 42, T, Some("x"), None),
					("B/sub/log.txt", 42, T + 1000, None, None),
				],
			)
			.await;
			let nested = store(
				&dir,
				"nested",
				&[
					("n.txt", 42, T, Some("n"), None),
					("twin.txt", 42, T, Some("t"), Some("T1")),
					("verified.txt", 42, T, Some("v"), Some("V")),
					("deep/d.txt", 42, T, Some("d"), None),
				],
			)
			.await;
			Self {
				_dir: dir,
				outer,
				nested,
			}
		}

		fn folder_a(&self, after: Option<&CompareCursor>) -> Folder {
			Folder::new(
				vec![
					(self.outer.clone(), reach("/vol", "A", "")),
					(self.nested.clone(), reach("/vol/A/m", "", "m")),
				],
				false,
				after,
			)
		}

		fn folder_b(&self, after: Option<&CompareCursor>) -> Folder {
			Folder::new(
				vec![(self.outer.clone(), reach("/vol", "B", ""))],
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
	/// its size, modification time, the sampled hash its content is keyed by
	/// and the integrity hash of a file read in full.
	async fn store(
		dir: &tempfile::TempDir,
		name: &str,
		files: &[(&str, i64, i64, Option<&str>, Option<&str>)],
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
		for (path, size, mtime, _, _) in files {
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

		for (path, size, _, sampled, integrity) in files {
			let Some(sampled) = sampled else {
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
					sampled_hash: Some(sampled.to_string()),
					integrity_hash: integrity.map(str::to_string),
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
		let (a_after, b_after) = resumes(by, show, after);
		let page = Page {
			show,
			limit,
			count: after.is_none(),
			device_slug: "dev".to_string(),
		};
		compare(
			by,
			fixture.folder_a(a_after),
			fixture.folder_b(b_after),
			&page,
		)
		.await
		.expect("compare")
	}

	/// Each listed entry as the paths it names beneath the volume, A's first
	/// where it has both.
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
			.map(|entry| match (named(&entry.a), named(&entry.b)) {
				(Some(a), Some(b)) => format!("{a} {b}"),
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
			resume("A", "", &at("sub", "c.txt")),
			after("A/sub", "c.txt")
		);
		assert_eq!(resume("A", "", &at("", "a.txt")), after("A", "a.txt"));
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

	/// By path a file's set is decided at its relative path: missing from one
	/// side, or present in both with the same or different bytes. Bytes
	/// compare by integrity hash where both sides have one, by sampled hash
	/// where both are hashed, and by size and modification time where either
	/// is not, so a file read in full on one side still matches its unread
	/// copy, and a sampled collision is told apart once both sides are read.
	#[tokio::test]
	async fn by_path_sorts_each_file_into_its_set() {
		let fixture = Fixture::new().await;
		let first = |show| page(&fixture, CompareBy::Path, show, 100, None);

		let only_a = first(CompareSet::OnlyA).await;
		assert_eq!(
			listed(&only_a),
			[
				"A/moved.txt",
				"A/m/n.txt",
				"A/m-2/f.txt",
				"A/m/deep/d.txt",
				"A/sub/c.txt",
			]
		);
		assert_eq!(
			paths(&only_a),
			[
				"moved.txt",
				"m/n.txt",
				"m-2/f.txt",
				"m/deep/d.txt",
				"sub/c.txt"
			]
		);
		assert_eq!(only_a.next, None);
		assert_eq!(
			only_a.totals,
			Some(CompareTotals {
				only_a: 5,
				only_b: 2,
				both: 3,
				different: 4,
				unhashed_a: 2,
				unhashed_b: 2,
			})
		);

		assert_eq!(
			listed(&first(CompareSet::OnlyB).await),
			["B/elsewhere/moved.txt", "B/sub/extra.txt"]
		);
		assert_eq!(
			listed(&first(CompareSet::Different).await),
			[
				"A/b.txt B/b.txt",
				"A/m/twin.txt B/m/twin.txt",
				"A/sub/edited.txt B/sub/edited.txt",
				"A/sub/log.txt B/sub/log.txt",
			]
		);
		assert_eq!(
			listed(&first(CompareSet::Both).await),
			[
				"A/a.txt B/a.txt",
				"A/raw.bin B/raw.bin",
				"A/m/verified.txt B/m/verified.txt",
			]
		);
	}

	/// By content a file's set is decided by whether its bytes sit anywhere
	/// beneath the other folder, matched by sampled hash with the integrity
	/// hashes deciding where both sides have them. A file in B whose bytes A
	/// holds is counted once, from A, and an unhashed file is counted apart.
	#[tokio::test]
	async fn by_content_finds_bytes_wherever_they_sit() {
		let fixture = Fixture::new().await;
		let first = |show| page(&fixture, CompareBy::Content, show, 100, None);

		let only_a = first(CompareSet::OnlyA).await;
		assert_eq!(
			listed(&only_a),
			[
				"A/b.txt",
				"A/m/n.txt",
				"A/m/twin.txt",
				"A/m/deep/d.txt",
				"A/sub/c.txt",
				"A/sub/edited.txt",
			]
		);
		assert_eq!(
			only_a.totals,
			Some(CompareTotals {
				only_a: 6,
				only_b: 4,
				both: 4,
				different: 0,
				unhashed_a: 2,
				unhashed_b: 2,
			})
		);

		let only_b = first(CompareSet::OnlyB).await;
		assert_eq!(
			listed(&only_b),
			[
				"B/b.txt",
				"B/m/twin.txt",
				"B/sub/edited.txt",
				"B/sub/extra.txt"
			]
		);
		assert_eq!(
			paths(&only_b),
			["b.txt", "m/twin.txt", "sub/edited.txt", "sub/extra.txt"]
		);
		assert_eq!(
			listed(&first(CompareSet::Both).await),
			["A/a.txt", "A/moved.txt", "A/m/verified.txt", "A/m-2/f.txt"]
		);
	}

	/// Following cursors page by page lists a set exactly as one page does,
	/// at page sizes that end pages inside the nested store, at its edge and
	/// between its root-level files and its directories.
	#[tokio::test]
	async fn pages_resume_where_the_last_ended() {
		let fixture = Fixture::new().await;
		let sets = [
			(CompareBy::Path, CompareSet::OnlyA),
			(CompareBy::Path, CompareSet::OnlyB),
			(CompareBy::Path, CompareSet::Both),
			(CompareBy::Path, CompareSet::Different),
			(CompareBy::Content, CompareSet::OnlyA),
			(CompareBy::Content, CompareSet::OnlyB),
			(CompareBy::Content, CompareSet::Both),
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

	/// A page's input is the comparison's fields and its own, flat on the
	/// wire, so a client that pages spells the comparison as an operation
	/// over one does.
	#[test]
	fn a_page_input_is_flat_on_the_wire() {
		let input: PathCompareInput = serde_json::from_value(serde_json::json!({
			"a": {"Physical": {"device_slug": "local", "path": "/a"}},
			"b": {"Physical": {"device_slug": "local", "path": "/b"}},
			"by": "content",
			"show": "both",
			"after": null,
			"limit": 10
		}))
		.expect("flat input");
		assert_eq!(input.comparison.by, CompareBy::Content);
		assert_eq!(input.comparison.show, CompareSet::Both);
		assert!(!input.comparison.include_hidden);
		assert_eq!(input.limit, 10);
	}

	#[test]
	fn a_page_asks_only_what_a_comparison_answers() {
		let input = |by, show, limit| PathCompareInput {
			comparison: Comparison {
				a: SdPath::local("/a"),
				b: SdPath::local("/b"),
				by,
				show,
				include_hidden: false,
			},
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
			PathCompareQuery::from_input(input(CompareBy::Path, CompareSet::Different, 100))
				.is_ok()
		);
		assert!(invalid(input(
			CompareBy::Content,
			CompareSet::Different,
			100
		)));
		assert!(invalid(input(CompareBy::Path, CompareSet::Both, 0)));
		assert!(invalid(input(
			CompareBy::Path,
			CompareSet::Both,
			MAX_PAGE + 1
		)));
	}
}
