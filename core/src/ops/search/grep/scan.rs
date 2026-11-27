//! The walk and search behind `search.grep`.
//!
//! Everything here is blocking filesystem work, so the query runs it on the
//! blocking pool. The walk uses the ripgrep crates: `ignore` for traversal and
//! ignore files, `grep-regex` and `grep-searcher` for matching lines.
//!
//! Files are visited depth first with each directory sorted by name, which
//! is the order of their relative paths. A cursor is the last file a page read
//! from and how many of its matches it returned, so the next page skips every
//! path before it without searching it again.

use std::{
	collections::VecDeque,
	io::{self, Read},
	path::{Path, PathBuf},
	sync::atomic::{AtomicBool, Ordering},
};

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use tracing::debug;

use super::{GrepMatch, GrepOptions, GrepOutput, GrepSkipReason, GrepSkipped};
use crate::ops::indexing::metadata::refuse_dataless_blocking;

/// Longest line text returned, so one minified file cannot fill a response.
const MAX_LINE_BYTES: usize = 4096;
/// Skipped files listed by name per page; the rest are only counted.
const MAX_SKIPPED_LISTED: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum GrepError {
	#[error("{0}")]
	Input(String),
	#[error("invalid pattern: {0}")]
	Pattern(String),
	#[error("invalid glob '{glob}': {message}")]
	Glob { glob: String, message: String },
	#[error("invalid cursor")]
	Cursor,
	#[error("cannot read scope '{path}': {source}")]
	Scope {
		path: String,
		#[source]
		source: io::Error,
	},
	#[error("search cancelled")]
	Cancelled,
}

/// One page of matches, built file by file in path order.
///
/// The local walk feeds it files from disk. A caller walking something else,
/// such as a bucket listing, feeds it the same way with [`Page::wants`] and
/// [`Page::search`], so both get the same matching, context, skip reasons and
/// cursors.
pub struct Page<'a> {
	matcher: RegexMatcher,
	searcher: Searcher,
	cursor: Option<Cursor>,
	max_results: usize,
	max_file_bytes: u64,
	context: usize,
	cancel: &'a AtomicBool,
	out: GrepOutput,
}

impl<'a> Page<'a> {
	pub fn new(options: &GrepOptions, cancel: &'a AtomicBool) -> Result<Self, GrepError> {
		options.validate().map_err(GrepError::Input)?;
		let matcher = RegexMatcherBuilder::new()
			.case_insensitive(options.case_insensitive)
			.fixed_strings(options.literal)
			.line_terminator(Some(b'\n'))
			.build(&options.pattern)
			.map_err(|e| GrepError::Pattern(e.to_string()))?;
		let context = options.context_lines as usize;
		let searcher = SearcherBuilder::new()
			.binary_detection(BinaryDetection::quit(b'\0'))
			.line_number(true)
			.before_context(context)
			.after_context(context)
			.build();
		Ok(Self {
			matcher,
			searcher,
			cursor: options.cursor.as_deref().map(Cursor::decode).transpose()?,
			max_results: options.max_results(),
			max_file_bytes: options.max_file_bytes(),
			context,
			cancel,
			out: GrepOutput::default(),
		})
	}

	/// Whether the file at `rel` belongs on this page: not before the cursor,
	/// and no larger than `max_file_bytes`. A file passed over for its size is
	/// counted as skipped.
	pub fn wants(&mut self, rel: &Path, size: u64, shown: &str) -> bool {
		if self
			.cursor
			.as_ref()
			.is_some_and(|cursor| rel < cursor.path.as_path())
		{
			return false;
		}
		if size > self.max_file_bytes {
			self.skip(shown, GrepSkipReason::TooLarge);
			return false;
		}
		true
	}

	/// The largest file searched.
	pub fn max_file_bytes(&self) -> u64 {
		self.max_file_bytes
	}

	pub fn skip(&mut self, shown: &str, reason: GrepSkipReason) {
		self.out.files_skipped += 1;
		if self.out.skipped.len() < MAX_SKIPPED_LISTED {
			self.out.skipped.push(GrepSkipped {
				path: shown.to_string(),
				reason,
			});
		}
	}

	/// Search one file's bytes. Answers true once the page is full, when the
	/// caller stops feeding it.
	pub fn search(&mut self, rel: &Path, shown: &str, bytes: &[u8]) -> Result<bool, GrepError> {
		let already_returned = match &self.cursor {
			Some(cursor) if cursor.path == rel => cursor.skip,
			_ => 0,
		};
		let remaining = self.max_results - self.out.matches.len();
		let mut sink = FileSink::new(self.context, already_returned, remaining, self.cancel);
		if let Err(error) = self.searcher.search_slice(&self.matcher, bytes, &mut sink) {
			debug!(path = %shown, %error, "grep search failed");
			return Ok(false);
		}
		if self.cancel.load(Ordering::Relaxed) {
			return Err(GrepError::Cancelled);
		}
		if sink.binary {
			self.skip(shown, GrepSkipReason::Binary);
			return Ok(false);
		}
		self.out.files_scanned += 1;

		let found = sink.found.len();
		self.out
			.matches
			.extend(sink.found.into_iter().map(|found| GrepMatch {
				path: shown.to_string(),
				line_number: found.line_number,
				line: found.line,
				before: found.before,
				after: found.after,
			}));
		if self.out.matches.len() < self.max_results {
			return Ok(false);
		}
		self.out.truncated = true;
		self.out.next_cursor = Some(
			Cursor {
				path: rel.to_path_buf(),
				skip: already_returned + found,
			}
			.encode(),
		);
		Ok(true)
	}

	pub fn finish(self) -> GrepOutput {
		self.out
	}
}

/// Search the files under `root.join(scope)` and return one page of matches.
///
/// `root` is trusted as given: a source root, or the subtree a caller may
/// read. `scope` is relative to it and is resolved from `root`'s handle with
/// no symlink allowed in any component. The scope's handle is opened once and
/// every file is opened beneath it, so renaming a directory and putting a
/// symlink in its place at any point during the scan cannot redirect a read
/// out of the scope. Files holding a NUL byte count as binary and are skipped,
/// as are files larger than `max_file_bytes`. Setting `cancel` stops the
/// search between files and between matches.
pub fn grep(
	root: &Path,
	scope: &Path,
	options: &GrepOptions,
	cancel: &AtomicBool,
) -> Result<GrepOutput, GrepError> {
	let mut page = Page::new(options, cancel)?;

	let anchor = Anchor::open(root, scope).map_err(|source| GrepError::Scope {
		path: root.join(scope).display().to_string(),
		source,
	})?;
	let scope = &root.join(scope);

	let mut walk = WalkBuilder::new(scope);
	walk.standard_filters(options.respect_gitignore)
		.hidden(!options.include_hidden)
		.require_git(false)
		.follow_links(false)
		.sort_by_file_name(|a, b| a.cmp(b));
	if !options.include.is_empty() || !options.exclude.is_empty() {
		let mut overrides = OverrideBuilder::new(scope);
		let globs = options
			.include
			.iter()
			.map(|glob| (glob, glob.clone()))
			.chain(
				options
					.exclude
					.iter()
					.map(|glob| (glob, format!("!{glob}"))),
			);
		for (glob, line) in globs {
			overrides.add(&line).map_err(|e| GrepError::Glob {
				glob: glob.clone(),
				message: e.to_string(),
			})?;
		}
		walk.overrides(overrides.build().map_err(|e| GrepError::Glob {
			glob: String::new(),
			message: e.to_string(),
		})?);
	}
	if let Some(cursor) = &page.cursor {
		// Prune everything before the cursor, keeping the directories on the
		// way down to it.
		let root = scope.to_path_buf();
		let resume = cursor.path.clone();
		walk.filter_entry(move |entry| {
			let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
			rel >= resume.as_path()
				|| (entry.file_type().is_some_and(|t| t.is_dir()) && resume.starts_with(rel))
		});
	}

	for entry in walk.build() {
		if cancel.load(Ordering::Relaxed) {
			return Err(GrepError::Cancelled);
		}
		let entry = match entry {
			Ok(entry) => entry,
			Err(error) => {
				// An unreadable directory or ignore file loses only its own subtree.
				debug!(%error, "grep walk error");
				continue;
			}
		};
		if !entry.file_type().is_some_and(|t| t.is_file()) {
			continue;
		}
		let path = entry.path();
		let rel = path.strip_prefix(scope).unwrap_or(path);
		let display = display_path(rel, path);

		// A stat, which the provider answers without downloading anything.
		if refuse_dataless_blocking(path).is_err() {
			page.skip(&display, GrepSkipReason::CloudPlaceholder);
			continue;
		}
		let bytes = match read_capped(&anchor, rel, page.max_file_bytes) {
			Ok(Some(bytes)) => bytes,
			Ok(None) => {
				page.skip(&display, GrepSkipReason::TooLarge);
				continue;
			}
			Err(error) => {
				debug!(path = %path.display(), %error, "grep skipped an unreadable file");
				continue;
			}
		};
		if page.search(rel, &display, &bytes)? {
			break;
		}
	}
	Ok(page.finish())
}

/// Read a regular file whole, or `None` when it is larger than `cap`.
fn read_capped(anchor: &Anchor, rel: &Path, cap: u64) -> io::Result<Option<Vec<u8>>> {
	let file = anchor.open_file(rel)?;
	let metadata = file.metadata()?;
	if !metadata.is_file() || metadata.len() > cap {
		return Ok(None);
	}
	let mut bytes = Vec::with_capacity(metadata.len() as usize);
	file.take(cap + 1).read_to_end(&mut bytes)?;
	if bytes.len() as u64 > cap {
		return Ok(None);
	}
	Ok(Some(bytes))
}

/// A handle on the scope, held for the whole scan, that files are opened
/// beneath one component at a time.
///
/// The walk lists a path and the open comes later. In between, any directory
/// on the way, the scope included, can be swapped for a symlink out of it. A
/// flag on the last component cannot see that, and reopening the scope by
/// path would follow it. Opening from a retained handle with `O_NOFOLLOW` on
/// every component cannot be redirected. `O_NONBLOCK` keeps a FIFO swapped in
/// for a file from blocking the read.
#[cfg(unix)]
struct Anchor {
	dir: std::os::fd::OwnedFd,
	/// Set when the scope is a single file in `dir`.
	file: Option<std::ffi::OsString>,
}

#[cfg(unix)]
impl Anchor {
	const LEAF: libc::c_int = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK;
	const DIR: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW;

	fn open(root: &Path, scope: &Path) -> io::Result<Self> {
		use std::os::unix::fs::OpenOptionsExt;

		let mut dir: std::os::fd::OwnedFd = std::fs::OpenOptions::new()
			.read(true)
			.custom_flags(libc::O_DIRECTORY)
			.open(root)?
			.into();
		let parts = normal_parts(scope)?;
		let Some((last, ancestors)) = parts.split_last() else {
			return Ok(Self { dir, file: None });
		};
		for part in ancestors {
			dir = open_at(&dir, part, Self::DIR)?;
		}
		// Decide by a stat rather than an open: opening a file scope that is a
		// cloud placeholder would download it before the scan can refuse it.
		match stat_at(&dir, last)?.st_mode & libc::S_IFMT {
			libc::S_IFDIR => Ok(Self {
				dir: open_at(&dir, last, Self::DIR)?,
				file: None,
			}),
			libc::S_IFLNK => Err(io::Error::other("the scope is a symlink")),
			_ => Ok(Self {
				dir,
				file: Some(last.to_os_string()),
			}),
		}
	}

	fn open_file(&self, rel: &Path) -> io::Result<std::fs::File> {
		if self.file.is_some() && !rel.as_os_str().is_empty() {
			return Err(io::Error::other("the scope is a single file"));
		}
		let parts = normal_parts(rel)?;
		let Some((leaf, ancestors)) = parts.split_last() else {
			let name = self
				.file
				.as_deref()
				.ok_or_else(|| io::Error::other("the scope is a directory"))?;
			return Ok(open_at(&self.dir, name, Self::LEAF)?.into());
		};
		let mut dir = None;
		for part in ancestors {
			dir = Some(open_at(dir.as_ref().unwrap_or(&self.dir), part, Self::DIR)?);
		}
		Ok(open_at(dir.as_ref().unwrap_or(&self.dir), leaf, Self::LEAF)?.into())
	}
}

#[cfg(unix)]
fn normal_parts(path: &Path) -> io::Result<Vec<&std::ffi::OsStr>> {
	path.components()
		.map(|part| match part {
			std::path::Component::Normal(name) => Ok(name),
			_ => Err(io::Error::other("path leaves the scope")),
		})
		.collect()
}

#[cfg(unix)]
fn open_at(
	dir: &std::os::fd::OwnedFd,
	name: &std::ffi::OsStr,
	flags: libc::c_int,
) -> io::Result<std::os::fd::OwnedFd> {
	use std::os::{
		fd::{AsRawFd, FromRawFd},
		unix::ffi::OsStrExt,
	};

	let name = std::ffi::CString::new(name.as_bytes())?;
	// SAFETY: `dir` is an open descriptor and `name` is NUL-terminated.
	let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags | libc::O_CLOEXEC) };
	if fd < 0 {
		return Err(io::Error::last_os_error());
	}
	// SAFETY: `fd` was just opened and nothing else owns it.
	Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) })
}

#[cfg(unix)]
fn stat_at(dir: &std::os::fd::OwnedFd, name: &std::ffi::OsStr) -> io::Result<libc::stat> {
	use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};

	let name = std::ffi::CString::new(name.as_bytes())?;
	let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
	// SAFETY: `dir` is an open descriptor, `name` is NUL-terminated and
	// `stat` is written by the call before it is read.
	let result = unsafe {
		libc::fstatat(
			dir.as_raw_fd(),
			name.as_ptr(),
			stat.as_mut_ptr(),
			libc::AT_SYMLINK_NOFOLLOW,
		)
	};
	if result != 0 {
		return Err(io::Error::last_os_error());
	}
	// SAFETY: fstatat succeeded, so it filled `stat`.
	Ok(unsafe { stat.assume_init() })
}

#[cfg(not(unix))]
struct Anchor {
	scope: PathBuf,
}

#[cfg(not(unix))]
impl Anchor {
	fn open(root: &Path, scope: &Path) -> io::Result<Self> {
		let scope = root.join(scope);
		std::fs::metadata(&scope)?;
		Ok(Self { scope })
	}

	fn open_file(&self, rel: &Path) -> io::Result<std::fs::File> {
		std::fs::File::open(self.scope.join(rel))
	}
}

/// A path relative to the scope with `/` separators, or the file's own name
/// when the scope is the file.
fn display_path(rel: &Path, full: &Path) -> String {
	if rel.as_os_str().is_empty() {
		return full
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
	}
	rel.components()
		.map(|part| part.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
}

fn line_text(bytes: &[u8]) -> String {
	let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
	let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
	let mut text = String::from_utf8_lossy(bytes).into_owned();
	if text.len() > MAX_LINE_BYTES {
		let mut end = MAX_LINE_BYTES;
		while !text.is_char_boundary(end) {
			end -= 1;
		}
		text.truncate(end);
	}
	text
}

struct Found {
	line_number: u64,
	line: String,
	before: Vec<String>,
	after: Vec<String>,
}

/// Collects one file's matches, each with its own context. Context is kept
/// per match rather than shared, so a line between two close matches is the
/// first one's after and the second one's before.
struct FileSink<'a> {
	context: usize,
	skip: usize,
	limit: usize,
	cancel: &'a AtomicBool,
	seen: usize,
	recent: VecDeque<(u64, String)>,
	found: Vec<Found>,
	binary: bool,
}

impl<'a> FileSink<'a> {
	fn new(context: usize, skip: usize, limit: usize, cancel: &'a AtomicBool) -> Self {
		Self {
			context,
			skip,
			limit,
			cancel,
			seen: 0,
			recent: VecDeque::new(),
			found: Vec::new(),
			binary: false,
		}
	}

	/// Hand a line to the matches still waiting for after-context, and keep it
	/// as possible before-context for the next match.
	fn push_line(&mut self, line_number: u64, text: &str) {
		for found in self.found.iter_mut().rev() {
			if line_number > found.line_number + self.context as u64 {
				break;
			}
			if line_number > found.line_number {
				found.after.push(text.to_string());
			}
		}
		if self.context > 0 {
			if self.recent.len() == self.context {
				self.recent.pop_front();
			}
			self.recent.push_back((line_number, text.to_string()));
		}
	}

	fn full(&self) -> bool {
		self.found.len() >= self.limit
			&& self
				.found
				.last()
				.is_none_or(|last| last.after.len() >= self.context)
	}
}

impl Sink for FileSink<'_> {
	type Error = io::Error;

	fn matched(&mut self, _: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
		if self.cancel.load(Ordering::Relaxed) {
			return Ok(false);
		}
		let line_number = mat.line_number().unwrap_or(0);
		let text = line_text(mat.bytes());
		if self.found.len() >= self.limit {
			self.push_line(line_number, &text);
			return Ok(!self.full());
		}
		self.seen += 1;
		if self.seen > self.skip {
			let first = line_number.saturating_sub(self.context as u64);
			let before = self
				.recent
				.iter()
				.filter(|(n, _)| *n >= first && *n < line_number)
				.map(|(_, line)| line.clone())
				.collect();
			self.push_line(line_number, &text);
			self.found.push(Found {
				line_number,
				line: text,
				before,
				after: Vec::new(),
			});
		} else {
			self.push_line(line_number, &text);
		}
		Ok(!self.full())
	}

	fn context(&mut self, _: &Searcher, ctx: &SinkContext<'_>) -> Result<bool, io::Error> {
		if self.cancel.load(Ordering::Relaxed) {
			return Ok(false);
		}
		self.push_line(ctx.line_number().unwrap_or(0), &line_text(ctx.bytes()));
		Ok(!self.full())
	}

	fn binary_data(&mut self, _: &Searcher, _: u64) -> Result<bool, io::Error> {
		self.binary = true;
		self.found.clear();
		Ok(false)
	}
}

/// Where a page stopped: the file it read last, relative to the scope, and how
/// many of that file's matches have been returned.
struct Cursor {
	path: PathBuf,
	skip: usize,
}

impl Cursor {
	fn encode(&self) -> String {
		format!("{}:{}", self.skip, hex::encode(path_bytes(&self.path)))
	}

	fn decode(raw: &str) -> Result<Self, GrepError> {
		let (skip, path) = raw.split_once(':').ok_or(GrepError::Cursor)?;
		Ok(Self {
			skip: skip.parse().map_err(|_| GrepError::Cursor)?,
			path: path_from_bytes(hex::decode(path).map_err(|_| GrepError::Cursor)?)?,
		})
	}
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
	use std::os::unix::ffi::OsStrExt;
	path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
	path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> Result<PathBuf, GrepError> {
	use std::os::unix::ffi::OsStringExt;
	Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> Result<PathBuf, GrepError> {
	String::from_utf8(bytes)
		.map(PathBuf::from)
		.map_err(|_| GrepError::Cursor)
}

#[cfg(all(test, unix))]
mod tests {
	use std::path::Path;

	use std::sync::atomic::AtomicBool;

	use super::{grep, read_capped, Anchor};
	use crate::ops::{
		indexing::metadata::dataless_for_tests,
		search::grep::{GrepOptions, GrepSkipReason, GrepSkipped},
	};

	/// `<tmp>/root/scope/sub/file.txt` reads "inside"; `<tmp>/outside` has
	/// the same shape and reads "outside".
	fn tree() -> tempfile::TempDir {
		let dir = tempfile::TempDir::new().unwrap();
		for (base, text) in [("root/scope", "inside"), ("outside", "outside")] {
			let sub = dir.path().join(base).join("sub");
			std::fs::create_dir_all(&sub).unwrap();
			std::fs::write(sub.join("file.txt"), text).unwrap();
		}
		dir
	}

	fn read(anchor: &Anchor, rel: &str) -> std::io::Result<Option<Vec<u8>>> {
		read_capped(anchor, Path::new(rel), 1024)
	}

	#[test]
	fn a_directory_swapped_for_a_symlink_mid_scan_is_refused() {
		let dir = tree();
		let root = dir.path().join("root");
		let anchor = Anchor::open(&root, Path::new("scope")).unwrap();
		assert_eq!(read(&anchor, "sub/file.txt").unwrap().unwrap(), b"inside");

		let scope = root.join("scope");
		std::fs::rename(scope.join("sub"), dir.path().join("moved")).unwrap();
		std::os::unix::fs::symlink(dir.path().join("outside/sub"), scope.join("sub")).unwrap();
		assert!(read(&anchor, "sub/file.txt").is_err());
	}

	#[test]
	fn the_scope_swapped_for_a_symlink_mid_scan_keeps_reading_the_original() {
		let dir = tree();
		let root = dir.path().join("root");
		let anchor = Anchor::open(&root, Path::new("scope")).unwrap();

		std::fs::rename(root.join("scope"), dir.path().join("moved")).unwrap();
		std::os::unix::fs::symlink(dir.path().join("outside"), root.join("scope")).unwrap();
		assert_eq!(read(&anchor, "sub/file.txt").unwrap().unwrap(), b"inside");
	}

	#[test]
	fn a_scope_reached_through_a_symlink_is_refused() {
		let dir = tree();
		let root = dir.path().join("root");
		std::os::unix::fs::symlink(dir.path().join("outside"), root.join("link")).unwrap();
		assert!(Anchor::open(&root, Path::new("link")).is_err());
		assert!(Anchor::open(&root, Path::new("link/sub")).is_err());
	}

	#[test]
	fn a_file_scope_reads_only_that_file() {
		let dir = tree();
		let root = dir.path().join("root");
		let anchor = Anchor::open(&root, Path::new("scope/sub/file.txt")).unwrap();
		assert_eq!(read(&anchor, "").unwrap().unwrap(), b"inside");

		// A file scope replaced by a directory must not read the file's
		// siblings, which sit in the directory the anchor holds.
		let sub = root.join("scope/sub");
		std::fs::write(sub.join("sibling.txt"), b"sibling").unwrap();
		std::fs::remove_file(sub.join("file.txt")).unwrap();
		std::fs::create_dir(sub.join("file.txt")).unwrap();
		std::fs::write(sub.join("file.txt/sibling.txt"), b"nested").unwrap();
		assert!(read(&anchor, "sibling.txt").is_err());
	}

	#[test]
	fn a_cloud_placeholder_is_skipped_without_being_opened() {
		use std::os::unix::fs::PermissionsExt;

		let dir = tree();
		let root = dir.path().join("root");
		let placeholder = root.join("scope/sub/placeholder.txt");
		std::fs::write(&placeholder, b"inside").unwrap();
		// Unreadable, so an open would fail as unreadable rather than count
		// as a placeholder.
		std::fs::set_permissions(&placeholder, std::fs::Permissions::from_mode(0o000)).unwrap();
		dataless_for_tests::mark(&placeholder);

		let out = grep(
			&root,
			Path::new("scope"),
			&GrepOptions::new("inside"),
			&AtomicBool::new(false),
		)
		.unwrap();
		assert_eq!(out.matches.len(), 1);
		assert_eq!(out.matches[0].path, "sub/file.txt");
		assert_eq!(out.files_skipped, 1);
		assert_eq!(
			out.skipped,
			vec![GrepSkipped {
				path: "sub/placeholder.txt".into(),
				reason: GrepSkipReason::CloudPlaceholder,
			}]
		);
	}

	#[test]
	fn a_file_scope_that_is_a_placeholder_is_never_opened() {
		use std::os::unix::fs::PermissionsExt;

		let dir = tree();
		let root = dir.path().join("root");
		let placeholder = root.join("scope/sub/only.txt");
		std::fs::write(&placeholder, b"inside").unwrap();
		std::fs::set_permissions(&placeholder, std::fs::Permissions::from_mode(0o000)).unwrap();
		dataless_for_tests::mark(&placeholder);

		let out = grep(
			&root,
			Path::new("scope/sub/only.txt"),
			&GrepOptions::new("inside"),
			&AtomicBool::new(false),
		)
		.unwrap();
		assert!(out.matches.is_empty());
		assert_eq!(
			out.skipped,
			vec![GrepSkipped {
				path: "only.txt".into(),
				reason: GrepSkipReason::CloudPlaceholder,
			}]
		);
	}
}
