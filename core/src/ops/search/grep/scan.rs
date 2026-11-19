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

use super::{GrepMatch, GrepOptions, GrepOutput};

/// Longest line text returned, so one minified file cannot fill a response.
const MAX_LINE_BYTES: usize = 4096;

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

/// Search the files under `scope` and return one page of matches.
///
/// `scope` is used as given: the caller decides which path may be read.
/// Symlinks below it are never followed, so the walk cannot leave it. Files
/// holding a NUL byte count as binary and are skipped, as are files larger
/// than `max_file_bytes`. Setting `cancel` stops the search between files
/// and between matches.
pub fn grep(
	scope: &Path,
	options: &GrepOptions,
	cancel: &AtomicBool,
) -> Result<GrepOutput, GrepError> {
	options.validate().map_err(GrepError::Input)?;
	let matcher = RegexMatcherBuilder::new()
		.case_insensitive(options.case_insensitive)
		.fixed_strings(options.literal)
		.line_terminator(Some(b'\n'))
		.build(&options.pattern)
		.map_err(|e| GrepError::Pattern(e.to_string()))?;
	let context = options.context_lines as usize;
	let mut searcher = SearcherBuilder::new()
		.binary_detection(BinaryDetection::quit(b'\0'))
		.line_number(true)
		.before_context(context)
		.after_context(context)
		.build();
	let cursor = options.cursor.as_deref().map(Cursor::decode).transpose()?;
	let max_results = options.max_results();
	let max_file_bytes = options.max_file_bytes();

	std::fs::symlink_metadata(scope).map_err(|source| GrepError::Scope {
		path: scope.display().to_string(),
		source,
	})?;

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
	if let Some(cursor) = &cursor {
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

	let mut out = GrepOutput::default();
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
		let already_returned = match &cursor {
			Some(cursor) if cursor.path == rel => cursor.skip,
			_ => 0,
		};

		let bytes = match read_capped(path, entry_ino(&entry), max_file_bytes) {
			Ok(Some(bytes)) => bytes,
			Ok(None) => {
				out.files_skipped += 1;
				continue;
			}
			Err(error) => {
				debug!(path = %path.display(), %error, "grep skipped an unreadable file");
				continue;
			}
		};

		let remaining = max_results - out.matches.len();
		let mut sink = FileSink::new(context, already_returned, remaining, cancel);
		if let Err(error) = searcher.search_slice(&matcher, &bytes, &mut sink) {
			debug!(path = %path.display(), %error, "grep search failed");
			continue;
		}
		if cancel.load(Ordering::Relaxed) {
			return Err(GrepError::Cancelled);
		}
		if sink.binary {
			out.files_skipped += 1;
			continue;
		}
		out.files_scanned += 1;

		let display = display_path(rel, path);
		let found = sink.found.len();
		out.matches
			.extend(sink.found.into_iter().map(|found| GrepMatch {
				path: display.clone(),
				line_number: found.line_number,
				line: found.line,
				before: found.before,
				after: found.after,
			}));
		if out.matches.len() >= max_results {
			out.truncated = true;
			out.next_cursor = Some(
				Cursor {
					path: rel.to_path_buf(),
					skip: already_returned + found,
				}
				.encode(),
			);
			break;
		}
	}
	Ok(out)
}

#[cfg(unix)]
fn entry_ino(entry: &ignore::DirEntry) -> Option<u64> {
	entry.ino()
}

#[cfg(not(unix))]
fn entry_ino(_entry: &ignore::DirEntry) -> Option<u64> {
	None
}

/// Read a regular file whole, or `None` when it is larger than `cap`.
///
/// `O_NOFOLLOW` guards only the last component. A directory above the file
/// can be swapped for a symlink between the walk and the open, so the opened
/// file must also be the inode the walk listed, or it is refused.
fn read_capped(path: &Path, expected_ino: Option<u64>, cap: u64) -> io::Result<Option<Vec<u8>>> {
	let mut open = std::fs::OpenOptions::new();
	open.read(true);
	#[cfg(unix)]
	{
		// The walk saw a regular file; refuse whatever replaced it since, a
		// symlink out of the scope or a FIFO that would block the read.
		use std::os::unix::fs::OpenOptionsExt;
		open.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
	}
	let file = open.open(path)?;
	let metadata = file.metadata()?;
	#[cfg(unix)]
	if let Some(expected) = expected_ino {
		use std::os::unix::fs::MetadataExt;
		if metadata.ino() != expected {
			return Err(io::Error::other("file changed since the walk listed it"));
		}
	}
	#[cfg(not(unix))]
	let _ = expected_ino;
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
	use std::os::unix::fs::MetadataExt;

	use super::read_capped;

	#[test]
	fn a_file_swapped_in_after_the_walk_is_refused() {
		let dir = tempfile::TempDir::new().unwrap();
		let listed = dir.path().join("listed.txt");
		let swapped = dir.path().join("swapped.txt");
		std::fs::write(&listed, b"needle\n").unwrap();
		std::fs::write(&swapped, b"needle\n").unwrap();
		let listed_ino = std::fs::metadata(&listed).unwrap().ino();

		assert!(read_capped(&listed, Some(listed_ino), 1024)
			.unwrap()
			.is_some());
		assert!(read_capped(&swapped, Some(listed_ino), 1024).is_err());
	}
}
