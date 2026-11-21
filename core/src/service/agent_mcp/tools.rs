//! # Agent MCP tools
//!
//! The curated tool set.
//!
//! A tool is a [`Tool`] variant with its name, the capability it needs, its
//! entry in [`definitions`] and its arm in [`call`]. The transport reads all of
//! that from here, so adding a tool touches only this file.
//!
//! Every tool takes a path relative to the grant root and works on the
//! [`GrantScope::path`] the grant check resolved, never on the path it was
//! sent. Walkers do not follow links. Every path in a result is relative to
//! the grant root, and error messages carry no host path.

use std::{
	fs::{File, Metadata},
	io::{BufRead, BufReader, Read, Seek, SeekFrom},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::SystemTime,
};

use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;

use super::{limits::*, paths, SKILL};
use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError},
	},
	ops::{
		agent_write::{
			self, ContentEncoding, DeleteInput, MakeDirInput, MoveInput, Precondition,
			WriteFileInput,
		},
		grants::{create::owning_source, GrantCap, GrantScope},
		indexing::{
			metadata::{refuse_dataless_blocking, DATALESS_REASON},
			text_extract::TextKind,
		},
		search::{
			arena_search::search_arena,
			content::{ContentSearchInput, ContentSearchQuery},
			grep::{scan, GrepOptions},
			input::{PaginationOptions, SearchFilters, SortDirection, SortField, SortOptions},
		},
	},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
	ListDir,
	Stat,
	Read,
	Glob,
	Grep,
	Search,
	SearchContent,
	GetSkill,
	WriteFile,
	MakeDir,
	Move,
	Delete,
}

impl Tool {
	pub const ALL: [Tool; 12] = [
		Self::GetSkill,
		Self::ListDir,
		Self::Stat,
		Self::Read,
		Self::Glob,
		Self::Grep,
		Self::Search,
		Self::SearchContent,
		Self::WriteFile,
		Self::MakeDir,
		Self::Move,
		Self::Delete,
	];

	pub fn name(self) -> &'static str {
		match self {
			Self::ListDir => "list_dir",
			Self::Stat => "stat",
			Self::Read => "read",
			Self::Glob => "glob",
			Self::Grep => "grep",
			Self::Search => "search",
			Self::SearchContent => "search_content",
			Self::GetSkill => "get_skill",
			Self::WriteFile => "write_file",
			Self::MakeDir => "make_dir",
			Self::Move => "move",
			Self::Delete => "delete",
		}
	}

	pub fn parse(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|tool| tool.name() == name)
	}

	/// The capability the grant check asks for on this tool's path.
	pub fn cap(self) -> GrantCap {
		match self {
			Self::WriteFile | Self::MakeDir | Self::Move | Self::Delete => GrantCap::ReadWrite,
			_ => GrantCap::Read,
		}
	}

	/// Whether a grant may run only one call of this tool at a time, for
	/// tools whose cost is a scan of the whole subtree.
	pub fn exclusive(self) -> bool {
		self == Self::Grep
	}
}

/// A tool's answer: the content blocks an agent reads, and the same facts as
/// structured content.
pub struct Output {
	content: Vec<Value>,
	structured: Value,
}

impl Output {
	/// The structured value, also as JSON text for clients that only read
	/// text content.
	fn json(structured: Value) -> Self {
		let text = serde_json::to_string_pretty(&structured).unwrap_or_default();
		Self {
			content: vec![json!({ "type": "text", "text": text })],
			structured,
		}
	}

	pub fn into_result(self) -> Value {
		json!({
			"content": self.content,
			"structuredContent": self.structured,
			"isError": false,
		})
	}
}

pub fn error_result(message: &str) -> Value {
	json!({
		"content": [{ "type": "text", "text": message }],
		"isError": true,
	})
}

/// The tool table `tools/list` answers with.
pub fn definitions() -> Value {
	let path = |what: &str| {
		json!({
			"type": "string",
			"description": format!("{what}, relative to the grant root. Use \"\" or \".\" for the root itself. No leading slash and no '..'."),
		})
	};
	let cursor = json!({
		"type": "string",
		"description": "The next_cursor of the previous page, to continue from there.",
	});
	let expect = json!({
		"type": "object",
		"description": "What you read: the content_hash and/or revision stat returned. Every field you pass must still match.",
		"properties": {
			"content_hash": { "type": "string" },
			"revision": { "type": "string" },
		},
		"additionalProperties": false,
	});
	let writes = |title: &str, destructive: bool| {
		json!({
			"title": title,
			"readOnlyHint": false,
			"destructiveHint": destructive,
			"idempotentHint": false,
			"openWorldHint": false,
		})
	};
	let annotations = |title: &str| {
		json!({
			"title": title,
			"readOnlyHint": true,
			"destructiveHint": false,
			"idempotentHint": true,
			"openWorldHint": false,
		})
	};
	json!([
		{
			"name": "get_skill",
			"title": "How to use this Spacedrive folder",
			"description": "Returns the guide for these tools: path rules, read caps, paging, and why grep and search can miss files. Call it once before using the other tools.",
			"inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
			"annotations": annotations("Read the usage guide"),
		},
		{
			"name": "list_dir",
			"title": "List a directory",
			"description": "Lists one directory of the granted folder, sorted by name, hidden entries included. Symlinks are listed as links and not followed. Pages hold up to 1000 entries.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The directory"),
					"limit": { "type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": PAGE_DEFAULT, "description": "Entries per page." },
					"cursor": cursor,
				},
				"additionalProperties": false,
			},
			"annotations": annotations("List a directory"),
		},
		{
			"name": "stat",
			"title": "Stat a path",
			"description": "Returns the type, size and timestamps of one file or directory, and the content_hash and revision a write's expect names.",
			"inputSchema": {
				"type": "object",
				"properties": { "path": path("The file or directory") },
				"required": ["path"],
				"additionalProperties": false,
			},
			"annotations": annotations("Stat a path"),
		},
		{
			"name": "read",
			"title": "Read a file",
			"description": "Reads part of a file, by byte range (offset, length) or by line range (start_line, line_count). One call returns at most 64 KiB or 2000 lines unless max_bytes raises the byte cap, up to 1 MiB. The result says whether it was cut short and where to continue. Text that is not UTF-8 is returned as base64.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The file"),
					"offset": { "type": "integer", "minimum": 0, "description": "Byte offset to start at. Default 0." },
					"length": { "type": "integer", "minimum": 1, "description": "Bytes to read. Capped by max_bytes." },
					"start_line": { "type": "integer", "minimum": 1, "description": "First line to return, 1-based. Selects line mode." },
					"line_count": { "type": "integer", "minimum": 1, "description": "Lines to return in line mode. Default 2000." },
					"max_bytes": { "type": "integer", "minimum": 1, "maximum": READ_MAX_BYTES, "default": READ_DEFAULT_BYTES, "description": "Byte cap for this call." },
				},
				"required": ["path"],
				"additionalProperties": false,
			},
			"annotations": annotations("Read a file"),
		},
		{
			"name": "glob",
			"title": "Find files by name pattern",
			"description": "Finds files and directories whose path, relative to 'path', matches a glob such as '**/*.rs' or 'docs/*.md'. Hidden files are included and .gitignore is respected unless told otherwise. A call visits at most 100,000 entries and says when it stopped early.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"pattern": { "type": "string", "description": "The glob. '*' stays within one directory, '**' crosses directories." },
					"path": path("The directory to search under"),
					"include_hidden": { "type": "boolean", "default": true },
					"respect_gitignore": { "type": "boolean", "default": true },
					"limit": { "type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": PAGE_DEFAULT },
					"cursor": cursor,
				},
				"required": ["pattern"],
				"additionalProperties": false,
			},
			"annotations": annotations("Find files by name pattern"),
		},
		{
			"name": "grep",
			"title": "Search file contents",
			"description": "Searches file contents under 'path' for a regular expression, or a literal string with literal=true. Hidden files are searched and .gitignore is respected by default. Binary files and files over 1 MiB are skipped and counted in files_skipped, so no match is not proof of absence. Pages hold up to 1000 matches.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"pattern": { "type": "string", "maxLength": PATTERN_MAX_BYTES, "description": "A regular expression (Rust regex syntax), or text with literal=true." },
					"path": path("The directory or file to search"),
					"literal": { "type": "boolean", "default": false },
					"case_insensitive": { "type": "boolean", "default": false },
					"include": { "type": "array", "items": { "type": "string" }, "description": "Globs a file must match, such as '*.rs'." },
					"exclude": { "type": "array", "items": { "type": "string" }, "description": "Globs that exclude files and directories." },
					"include_hidden": { "type": "boolean", "default": true },
					"respect_gitignore": { "type": "boolean", "default": true },
					"context_lines": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0 },
					"max_results": { "type": "integer", "minimum": 1, "maximum": GREP_PAGE_MAX, "default": 100 },
					"cursor": cursor,
				},
				"required": ["pattern"],
				"additionalProperties": false,
			},
			"annotations": annotations("Search file contents"),
		},
		{
			"name": "search",
			"title": "Search names and metadata",
			"description": "Searches Spacedrive's index of the granted folder by file name and metadata. It is fast but only knows what has been indexed, so a new or unindexed file may be missing. Confirm a hit with stat or read.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"query": { "type": "string", "description": "Words to match against file names." },
					"path": path("The directory to search under"),
					"limit": { "type": "integer", "minimum": 1, "maximum": SEARCH_PAGE_MAX, "default": SEARCH_PAGE_DEFAULT },
					"offset": { "type": "integer", "minimum": 0, "default": 0 },
				},
				"required": ["query"],
				"additionalProperties": false,
			},
			"annotations": annotations("Search names and metadata"),
		},
		{
			"name": "search_content",
			"title": "Search the text of files",
			"description": "Searches the text Spacedrive extracted from documents and code under 'path' and returns files ranked by relevance, each with a snippet around the best match. Every word must appear; text in double quotes is a phrase and a word ending in '*' matches as a prefix. It answers from an index, so a file changed since the last pass can be missing or stale, and pending_files counts files waiting to be read. Words spoken in transcribed audio and video match too: those hits have kind 'transcript' and carry start_seconds and end_seconds, the moment in the recording they were said, and one recording can return several. It works only inside a source that reads file contents. Use grep for patterns or a live scan.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"query": { "type": "string", "maxLength": PATTERN_MAX_BYTES, "description": "Words to find, such as 'budget \"quarterly report\"' or 'deploy*'." },
					"path": path("The directory or file to search"),
					"kinds": {
						"type": "array",
						"items": { "type": "string", "enum": ["text", "markdown", "csv", "json", "code", "pdf", "docx", "transcript"] },
						"description": "Only files of these kinds. Empty or absent means every kind.",
					},
					"max_results": { "type": "integer", "minimum": 1, "maximum": CONTENT_PAGE_MAX, "default": CONTENT_PAGE_DEFAULT },
					"cursor": cursor,
				},
				"required": ["query"],
				"additionalProperties": false,
			},
			"annotations": annotations("Search the text of files"),
		},
		{
			"name": "write_file",
			"title": "Write a file",
			"description": "Writes a whole file, or content at a byte offset of an existing one. To change an existing file, pass expect with the content_hash (or revision) that stat returned; if the file changed since, the call fails as stale and names the current values. Without expect the call creates the file and never replaces anything: if the name is taken, the file is written beside it as 'name (agent conflict N).ext'. Missing parent directories are created. The result has the new content_hash to pass to the next write, and an undo id.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The file"),
					"content": { "type": "string", "description": "The bytes to write, as UTF-8 text or base64." },
					"encoding": { "type": "string", "enum": ["utf8", "base64"], "default": "utf8" },
					"offset": { "type": "integer", "minimum": 0, "description": "Write content starting at this byte and keep the rest of the file. Needs expect." },
					"expect": expect.clone(),
				},
				"required": ["path", "content"],
				"additionalProperties": false,
			},
			"annotations": writes("Write a file", false),
		},
		{
			"name": "make_dir",
			"title": "Make a directory",
			"description": "Creates a directory and any missing parents. A directory already there is left as it is. If a file has the name, the directory is made beside it under a conflict name.",
			"inputSchema": {
				"type": "object",
				"properties": { "path": path("The directory") },
				"required": ["path"],
				"additionalProperties": false,
			},
			"annotations": writes("Make a directory", false),
		},
		{
			"name": "move",
			"title": "Move or rename",
			"description": "Moves or renames a file or directory within the granted folder and on one volume. expect names what stat returned for 'path'. If 'to' is taken, the item lands beside it under a conflict name. A directory that holds a protected file cannot be moved.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The file or directory to move"),
					"to": path("Where it goes"),
					"expect": expect.clone(),
				},
				"required": ["path", "to", "expect"],
				"additionalProperties": false,
			},
			"annotations": writes("Move or rename", true),
		},
		{
			"name": "delete",
			"title": "Move to the trash",
			"description": "Moves a file or directory to the trash. Nothing is deleted for good. expect names what stat returned for 'path'. A directory that holds a protected file cannot be deleted.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The file or directory"),
					"expect": expect,
				},
				"required": ["path", "expect"],
				"additionalProperties": false,
			},
			"annotations": writes("Move to the trash", true),
		},
	])
}

/// Run one tool on the resolved scope.
pub async fn call(
	context: Arc<CoreContext>,
	token: &str,
	tool: Tool,
	scope: GrantScope,
	arguments: Value,
	cancel: Arc<AtomicBool>,
) -> Result<Output, String> {
	let root = scope.root.clone();
	let result = match tool {
		Tool::GetSkill => Ok(Output {
			content: vec![json!({ "type": "text", "text": SKILL })],
			structured: json!({ "name": "spacedrive", "format": "markdown" }),
		}),
		Tool::Search => search(&context, &scope, args(arguments)?).await,
		Tool::SearchContent => search_content(&context, &scope, args(arguments)?).await,
		Tool::ListDir => blocking(move || list_dir(&scope, args(arguments)?)).await,
		Tool::Stat => stat_with_precondition(&context, scope, cancel).await,
		Tool::Read => blocking(move || read(&scope, args(arguments)?, &cancel)).await,
		Tool::Glob => blocking(move || glob(&scope, args(arguments)?, &cancel)).await,
		Tool::Grep => blocking(move || grep(&scope, args(arguments)?, &cancel)).await,
		Tool::WriteFile | Tool::MakeDir | Tool::Move | Tool::Delete => {
			write(context.clone(), token, tool, &scope, arguments, cancel).await
		}
	};
	result.map_err(|message| scrub(&message, &root))
}

/// Drop any spelling of the grant root from a message, so a lower layer's
/// error never shows the agent an absolute host path.
fn scrub(message: &str, root: &Path) -> String {
	let root = root.display().to_string();
	if root.is_empty() {
		return message.to_string();
	}
	message.replace(&format!("{root}/"), "").replace(&root, ".")
}

fn args<T: for<'de> Deserialize<'de>>(arguments: Value) -> Result<T, String> {
	serde_json::from_value(arguments).map_err(|e| format!("invalid arguments: {e}"))
}

async fn blocking(
	work: impl FnOnce() -> Result<Output, String> + Send + 'static,
) -> Result<Output, String> {
	tokio::task::spawn_blocking(work)
		.await
		.map_err(|_| "the call failed".to_string())?
}

fn io_message(error: &std::io::Error) -> String {
	use std::io::ErrorKind::*;
	match error.kind() {
		NotFound => "no such file or directory".to_string(),
		PermissionDenied => "permission denied".to_string(),
		NotADirectory => "not a directory".to_string(),
		IsADirectory => "is a directory".to_string(),
		_ => format!("I/O error: {}", error.kind()),
	}
}

fn kind(meta: &Metadata) -> &'static str {
	let file_type = meta.file_type();
	if file_type.is_symlink() {
		"symlink"
	} else if file_type.is_dir() {
		"dir"
	} else if file_type.is_file() {
		"file"
	} else {
		"other"
	}
}

fn time(at: std::io::Result<SystemTime>) -> Option<String> {
	at.ok().map(|at| DateTime::<Utc>::from(at).to_rfc3339())
}

fn page_limit(limit: Option<u32>, default: u32, max: u32) -> usize {
	limit.unwrap_or(default).clamp(1, max) as usize
}

fn page_offset(cursor: Option<&str>) -> Result<usize, String> {
	cursor
		.map(|c| c.parse::<usize>().map_err(|_| "invalid cursor".to_string()))
		.transpose()
		.map(Option::unwrap_or_default)
}

#[derive(Deserialize)]
struct ListArgs {
	#[serde(default)]
	limit: Option<u32>,
	#[serde(default)]
	cursor: Option<String>,
}

fn list_dir(scope: &GrantScope, args: ListArgs) -> Result<Output, String> {
	let meta = std::fs::metadata(&scope.path).map_err(|e| io_message(&e))?;
	if !meta.is_dir() {
		return Err("not a directory".to_string());
	}
	let mut names: Vec<_> = std::fs::read_dir(&scope.path)
		.map_err(|e| io_message(&e))?
		.filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
		.collect();
	names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));

	let offset = page_offset(args.cursor.as_deref())?;
	let limit = page_limit(args.limit, PAGE_DEFAULT, PAGE_MAX);
	let entries: Vec<Value> = names
		.iter()
		.skip(offset)
		.take(limit)
		.map(|name| {
			let meta = std::fs::symlink_metadata(scope.path.join(name)).ok();
			let shown = name.to_string_lossy();
			json!({
				"name": shown,
				"path": paths::joined(&scope.relative, &shown),
				"type": meta.as_ref().map(kind).unwrap_or("other"),
				"size": meta.as_ref().map(Metadata::len),
				"modified": meta.as_ref().and_then(|m| time(m.modified())),
			})
		})
		.collect();
	let next = offset + entries.len();
	Ok(Output::json(json!({
		"path": paths::shown(&scope.relative),
		"entries": entries,
		"total": names.len(),
		"next_cursor": (next < names.len()).then(|| next.to_string()),
	})))
}

fn stat(scope: &GrantScope) -> Result<Output, String> {
	let meta = std::fs::metadata(&scope.path).map_err(|e| io_message(&e))?;
	Ok(Output::json(json!({
		"path": paths::shown(&scope.relative),
		"type": kind(&meta),
		"size": meta.len(),
		"modified": time(meta.modified()),
		"created": time(meta.created()),
		"accessed": time(meta.accessed()),
		"readonly": meta.permissions().readonly(),
	})))
}

#[derive(Deserialize)]
struct ReadArgs {
	#[serde(default)]
	offset: Option<u64>,
	#[serde(default)]
	length: Option<u64>,
	#[serde(default)]
	start_line: Option<u64>,
	#[serde(default)]
	line_count: Option<u64>,
	#[serde(default)]
	max_bytes: Option<u64>,
}

/// Open a regular file without following a final symlink, which closes the
/// gap between the grant check and the open. A FIFO opened non-blocking
/// cannot stall the call.
fn open_file(path: &Path) -> Result<(File, Metadata), String> {
	let mut options = std::fs::OpenOptions::new();
	options.read(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
	}
	let file = options.open(path).map_err(|e| io_message(&e))?;
	let meta = file.metadata().map_err(|e| io_message(&e))?;
	if !meta.is_file() {
		return Err("not a regular file".to_string());
	}
	Ok((file, meta))
}

/// `bytes` as text when it is UTF-8, allowing one character cut at the end of
/// the range, otherwise as base64. Returns the content, its encoding, and
/// how many of the bytes it covers.
fn encode(bytes: &[u8]) -> (String, &'static str, usize) {
	match std::str::from_utf8(bytes) {
		Ok(text) => (text.to_string(), "utf-8", bytes.len()),
		Err(e) if e.error_len().is_none() && e.valid_up_to() > 0 => {
			let end = e.valid_up_to();
			(
				String::from_utf8_lossy(&bytes[..end]).into_owned(),
				"utf-8",
				end,
			)
		}
		Err(_) => (
			base64::engine::general_purpose::STANDARD.encode(bytes),
			"base64",
			bytes.len(),
		),
	}
}

fn read(scope: &GrantScope, args: ReadArgs, cancel: &AtomicBool) -> Result<Output, String> {
	let cap = args
		.max_bytes
		.unwrap_or(READ_DEFAULT_BYTES)
		.clamp(1, READ_MAX_BYTES);
	if args.start_line.is_some() && (args.offset.is_some() || args.length.is_some()) {
		return Err("use either offset and length, or start_line and line_count".to_string());
	}
	// A stat the provider answers locally; opening a placeholder would download it.
	if refuse_dataless_blocking(&scope.path).is_err() {
		return Err(format!(
			"the file is a cloud placeholder ({DATALESS_REASON}); it is not read so it is not downloaded"
		));
	}
	let (mut file, meta) = open_file(&scope.path)?;
	let size = meta.len();
	let path = paths::shown(&scope.relative);

	if let Some(start_line) = args.start_line {
		let count = args.line_count.unwrap_or(READ_DEFAULT_LINES).max(1);
		let lines = read_lines(file, start_line.max(1), count, cap, cancel)?;
		let structured = json!({
			"path": path,
			"size": size,
			"start_line": lines.first,
			"end_line": (lines.returned > 0).then(|| lines.first + lines.returned - 1),
			"lines_returned": lines.returned,
			"eof": lines.eof,
			"truncated": lines.truncated,
			"next_line": lines.next_line,
			"next_offset": lines.next_offset,
			"encoding": "utf-8",
		});
		return Ok(Output {
			content: vec![
				json!({ "type": "text", "text": String::from_utf8_lossy(&lines.content) }),
				json!({ "type": "text", "text": structured.to_string() }),
			],
			structured,
		});
	}

	let offset = args.offset.unwrap_or(0);
	let want = args.length.unwrap_or(cap).min(cap);
	file.seek(SeekFrom::Start(offset))
		.map_err(|e| io_message(&e))?;
	let mut bytes = Vec::with_capacity(want.min(size.saturating_sub(offset)) as usize);
	file.take(want)
		.read_to_end(&mut bytes)
		.map_err(|e| io_message(&e))?;
	let (text, encoding, covered) = encode(&bytes);
	let end = offset + covered as u64;
	let eof = end >= size;
	let truncated = !eof && args.length.map_or(true, |length| (covered as u64) < length);
	let structured = json!({
		"path": path,
		"size": size,
		"offset": offset,
		"bytes_read": covered,
		"eof": eof,
		"truncated": truncated,
		"next_offset": (!eof).then_some(end),
		"encoding": encoding,
	});
	Ok(Output {
		content: vec![
			json!({ "type": "text", "text": text }),
			json!({ "type": "text", "text": structured.to_string() }),
		],
		structured,
	})
}

/// A line range read without holding more than the byte cap.
struct Lines {
	content: Vec<u8>,
	first: u64,
	/// Lines returned whole, counting a final line with no terminator.
	returned: u64,
	eof: bool,
	truncated: bool,
	/// Where to continue in line mode, when the read stopped between lines.
	next_line: Option<u64>,
	/// Where to continue in byte mode, when the cap cut a line.
	next_offset: Option<u64>,
}

/// Read `count` lines from `start_line` in bounded chunks.
///
/// Skipped lines are scanned and dropped, never collected, and no line is
/// held beyond `cap`, so a file without newlines costs one buffer however
/// large it is. Cancellation is checked between chunks.
fn read_lines(
	file: File,
	start_line: u64,
	count: u64,
	cap: u64,
	cancel: &AtomicBool,
) -> Result<Lines, String> {
	let mut reader = BufReader::with_capacity(64 * 1024, file);
	let mut position = 0u64;
	let mut line = 1u64;
	while line < start_line {
		if cancel.load(Ordering::Relaxed) {
			return Err("cancelled".to_string());
		}
		let buf = reader.fill_buf().map_err(|e| io_message(&e))?;
		if buf.is_empty() {
			break;
		}
		let used = match buf.iter().position(|&b| b == b'\n') {
			Some(i) => {
				line += 1;
				i + 1
			}
			None => buf.len(),
		};
		reader.consume(used);
		position += used as u64;
	}

	let first = line;
	let mut content = Vec::new();
	let mut returned = 0u64;
	let mut at_line_start = true;
	let mut truncated = false;
	let mut eof = false;
	while returned < count {
		if cancel.load(Ordering::Relaxed) {
			return Err("cancelled".to_string());
		}
		let buf = reader.fill_buf().map_err(|e| io_message(&e))?;
		if buf.is_empty() {
			eof = true;
			break;
		}
		let room = (cap as usize).saturating_sub(content.len());
		if room == 0 {
			truncated = true;
			break;
		}
		let (chunk, ends_line) = match buf.iter().position(|&b| b == b'\n') {
			Some(i) => (i + 1, true),
			None => (buf.len(), false),
		};
		let taken = chunk.min(room);
		content.extend_from_slice(&buf[..taken]);
		reader.consume(taken);
		position += taken as u64;
		if taken < chunk {
			at_line_start = false;
			truncated = true;
			break;
		}
		at_line_start = ends_line;
		if ends_line {
			returned += 1;
		}
	}
	if !eof {
		eof = reader.fill_buf().map_err(|e| io_message(&e))?.is_empty();
	}
	if eof && !at_line_start {
		returned += 1;
		at_line_start = true;
	}
	Ok(Lines {
		content,
		first,
		returned,
		eof,
		truncated,
		next_line: (!eof && at_line_start).then_some(first + returned),
		next_offset: (!eof && !at_line_start).then_some(position),
	})
}

#[derive(Deserialize)]
struct GlobArgs {
	pattern: String,
	#[serde(default = "yes")]
	include_hidden: bool,
	#[serde(default = "yes")]
	respect_gitignore: bool,
	#[serde(default)]
	limit: Option<u32>,
	#[serde(default)]
	cursor: Option<String>,
}

fn yes() -> bool {
	true
}

fn glob(scope: &GrantScope, args: GlobArgs, cancel: &AtomicBool) -> Result<Output, String> {
	if args.pattern.is_empty() || args.pattern.len() > PATTERN_MAX_BYTES {
		return Err(format!(
			"the pattern must be between 1 and {PATTERN_MAX_BYTES} bytes"
		));
	}
	let matcher = globset::GlobBuilder::new(&args.pattern)
		.literal_separator(true)
		.build()
		.map_err(|e| format!("invalid glob: {}", e.kind()))?
		.compile_matcher();
	if !std::fs::metadata(&scope.path)
		.map_err(|e| io_message(&e))?
		.is_dir()
	{
		return Err("not a directory".to_string());
	}
	let offset = page_offset(args.cursor.as_deref())?;
	let limit = page_limit(args.limit, PAGE_DEFAULT, PAGE_MAX);

	let mut walk = ignore::WalkBuilder::new(&scope.path);
	walk.standard_filters(args.respect_gitignore)
		.hidden(!args.include_hidden)
		.require_git(false)
		.follow_links(false)
		.sort_by_file_name(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));

	let mut visited = 0u64;
	let mut seen = 0usize;
	let mut matches = Vec::new();
	let mut more = false;
	let mut stopped = false;
	for entry in walk.build() {
		if cancel.load(Ordering::Relaxed) {
			return Err("cancelled".to_string());
		}
		let Ok(entry) = entry else { continue };
		if entry.depth() == 0 {
			continue;
		}
		visited += 1;
		if visited > GLOB_VISIT_MAX {
			stopped = true;
			break;
		}
		let Ok(relative) = entry.path().strip_prefix(&scope.path) else {
			continue;
		};
		if !matcher.is_match(relative) {
			continue;
		}
		if seen >= offset {
			if matches.len() == limit {
				more = true;
				break;
			}
			let file_type = entry.file_type();
			matches.push(json!({
				"path": paths::joined(&scope.relative, &paths::shown(relative)),
				"type": match file_type {
					Some(t) if t.is_symlink() => "symlink",
					Some(t) if t.is_dir() => "dir",
					Some(t) if t.is_file() => "file",
					_ => "other",
				},
			}));
		}
		seen += 1;
	}
	let next = offset + matches.len();
	Ok(Output::json(json!({
		"matches": matches,
		// Past the visit limit a re-walk only reaches the same entries, so a
		// cursor is offered while pages still return matches.
		"next_cursor": (more || (stopped && !matches.is_empty())).then(|| next.to_string()),
		"visit_limit_reached": stopped,
		"entries_visited": visited.min(GLOB_VISIT_MAX),
	})))
}

#[derive(Deserialize)]
struct GrepArgs {
	pattern: String,
	#[serde(default)]
	literal: bool,
	#[serde(default)]
	case_insensitive: bool,
	#[serde(default)]
	include: Vec<String>,
	#[serde(default)]
	exclude: Vec<String>,
	#[serde(default = "yes")]
	include_hidden: bool,
	#[serde(default = "yes")]
	respect_gitignore: bool,
	#[serde(default)]
	context_lines: u32,
	#[serde(default)]
	max_results: Option<u32>,
	#[serde(default)]
	cursor: Option<String>,
}

fn grep(scope: &GrantScope, args: GrepArgs, cancel: &AtomicBool) -> Result<Output, String> {
	if args.pattern.len() > PATTERN_MAX_BYTES {
		return Err(format!(
			"the pattern is longer than {PATTERN_MAX_BYTES} bytes"
		));
	}
	if !args.literal {
		regex::RegexBuilder::new(&args.pattern)
			.case_insensitive(args.case_insensitive)
			.size_limit(REGEX_MAX_BYTES)
			.build()
			.map_err(|e| match e {
				regex::Error::CompiledTooBig(_) => {
					"the pattern compiles to more than 1 MiB; simplify it".to_string()
				}
				other => format!("invalid pattern: {other}"),
			})?;
	}
	let options = GrepOptions {
		pattern: args.pattern,
		literal: args.literal,
		case_insensitive: args.case_insensitive,
		include: args.include,
		exclude: args.exclude,
		respect_gitignore: args.respect_gitignore,
		include_hidden: args.include_hidden,
		context_lines: args.context_lines,
		max_results: Some(args.max_results.unwrap_or(100).clamp(1, GREP_PAGE_MAX)),
		cursor: args.cursor,
		max_file_bytes: None,
	};
	let out = scan::grep(&scope.root, &scope.relative, &options, cancel).map_err(|e| match e {
		scan::GrepError::Scope { source, .. } => io_message(&source),
		other => other.to_string(),
	})?;
	// A file scope names its matches by file name, so they join to its parent.
	let base = if scope.path.is_file() {
		scope.relative.parent().unwrap_or(Path::new(""))
	} else {
		scope.relative.as_path()
	};
	let matches: Vec<Value> = out
		.matches
		.into_iter()
		.map(|m| {
			json!({
				"path": paths::joined(base, &m.path),
				"line_number": m.line_number,
				"line": m.line,
				"before": m.before,
				"after": m.after,
			})
		})
		.collect();
	Ok(Output::json(json!({
		"matches": matches,
		"files_scanned": out.files_scanned,
		"files_skipped": out.files_skipped,
		"skipped": out.skipped.into_iter().map(|skipped| json!({
			"path": paths::joined(base, &skipped.path),
			"reason": skipped.reason,
		})).collect::<Vec<_>>(),
		"truncated": out.truncated,
		"next_cursor": out.next_cursor,
	})))
}

#[derive(Deserialize)]
struct SearchArgs {
	query: String,
	#[serde(default)]
	limit: Option<u32>,
	#[serde(default)]
	offset: Option<u32>,
}

async fn search(
	context: &Arc<CoreContext>,
	scope: &GrantScope,
	args: SearchArgs,
) -> Result<Output, String> {
	let limit = args
		.limit
		.unwrap_or(SEARCH_PAGE_DEFAULT)
		.clamp(1, SEARCH_PAGE_MAX);
	let offset = args.offset.unwrap_or(0);
	let filters = SearchFilters {
		include_hidden: Some(true),
		..SearchFilters::default()
	};
	let sort = SortOptions {
		field: SortField::Relevance,
		direction: SortDirection::Desc,
	};
	let pagination = PaginationOptions { limit, offset };
	let page = search_arena(
		&args.query,
		&SdPath::local(scope.path.clone()),
		&filters,
		&sort,
		&pagination,
		context,
		context.volume_index(),
		&context.file_type_registry(),
	)
	.await
	.map_err(|e| format!("search failed: {e}"))?;
	let total = page.total;
	let hits: Vec<_> = page
		.results
		.into_iter()
		.filter_map(|result| match &result.file.sd_path {
			SdPath::Physical { path, .. } => Some((path.clone(), result.file)),
			_ => None,
		})
		.collect();

	// The index can lag the disk, so each hit is placed by where it is now:
	// one that moved out of the grant, or whose link resolves out of it, is
	// dropped.
	let root = scope.root.clone();
	let placed: Vec<Option<PathBuf>> = {
		let paths: Vec<PathBuf> = hits.iter().map(|(path, _)| path.clone()).collect();
		tokio::task::spawn_blocking(move || {
			paths
				.iter()
				.map(|path| {
					std::fs::symlink_metadata(path).ok()?;
					crate::ops::grants::scope::resolve(&root, path)
						.ok()
						.map(|resolved| resolved.relative)
				})
				.collect()
		})
		.await
		.map_err(|_| "search failed".to_string())?
	};
	let results: Vec<Value> = hits
		.into_iter()
		.zip(placed)
		.filter_map(|((_, file), relative)| {
			let relative = relative?;
			Some(json!({
				"path": paths::shown(&relative),
				"name": file.name,
				"extension": file.extension,
				"kind": format!("{:?}", file.kind).to_lowercase(),
				"size": file.size,
				"modified": file.modified_at.to_rfc3339(),
			}))
		})
		.collect();
	Ok(Output::json(json!({
		"results": results,
		"total_found": total,
		"next_offset": (u64::from(offset + limit) < total).then_some(offset + limit),
		"note": "Search reads Spacedrive's index, which can lag the disk. Confirm a hit with stat or read.",
	})))
}

/// Stat, plus the values a write's precondition names.
async fn stat_with_precondition(
	context: &CoreContext,
	scope: GrantScope,
	cancel: Arc<AtomicBool>,
) -> Result<Output, String> {
	let (root, path) = (scope.root.clone(), scope.path.clone());
	let Output { mut structured, .. } = blocking(move || stat(&scope)).await?;
	let state = agent_write::state_of(context, &root, &path, cancel)
		.await
		.map_err(|e| e.to_string())?;
	structured["content_hash"] = json!(state.content_hash);
	structured["revision"] = json!(state.revision);
	Ok(Output::json(structured))
}

#[derive(Deserialize)]
struct WriteArgs {
	content: String,
	#[serde(default)]
	encoding: Option<ContentEncoding>,
	#[serde(default)]
	offset: Option<u64>,
	#[serde(default)]
	expect: Option<Precondition>,
}

#[derive(Deserialize)]
struct MoveArgs {
	to: String,
	expect: Precondition,
}

#[derive(Deserialize)]
struct DeleteArgs {
	expect: Precondition,
}

/// The write tools call the same core operations as the `agent.*` actions,
/// which check the token again for every path they touch, the destination of
/// a move included.
///
/// The write waits for its turn here, where a timeout or a revoked grant
/// still drops it from the queue, and then runs as its own task holding the
/// turn. That keeps at most one write running past its call. The transport
/// sets `cancel` when it gives up, which stops the write before its first
/// change; after that the task carries on to its commit or rollback and its
/// journal row, since a write dropped between setting the old bytes aside
/// and its rename would lose them. Only the answer is lost.
async fn write(
	context: Arc<CoreContext>,
	token: &str,
	tool: Tool,
	scope: &GrantScope,
	arguments: Value,
	cancel: Arc<AtomicBool>,
) -> Result<Output, String> {
	let token = token.to_string();
	let path = scope.relative.clone();
	enum Request {
		Write(WriteFileInput),
		MakeDir(MakeDirInput),
		Move(MoveInput),
		Delete(DeleteInput),
	}
	let request = match tool {
		Tool::WriteFile => {
			let args: WriteArgs = args(arguments)?;
			Request::Write(WriteFileInput {
				token,
				path,
				content: args.content,
				encoding: args.encoding,
				offset: args.offset,
				expect: args.expect,
			})
		}
		Tool::MakeDir => Request::MakeDir(MakeDirInput { token, path }),
		Tool::Move => {
			let args: MoveArgs = args(arguments)?;
			paths::check(&args.to)?;
			Request::Move(MoveInput {
				token,
				from: path,
				to: PathBuf::from(args.to),
				expect: args.expect,
			})
		}
		Tool::Delete => {
			let args: DeleteArgs = args(arguments)?;
			Request::Delete(DeleteInput {
				token,
				path,
				expect: args.expect,
			})
		}
		_ => return Err("not a write tool".to_string()),
	};
	let turn = agent_write::WriteTurn::take(cancel).await;
	let task = tokio::spawn(async move {
		match request {
			Request::Write(input) => agent_write::write_file_in(&context, input, turn).await,
			Request::MakeDir(input) => agent_write::make_dir_in(&context, input, turn).await,
			Request::Move(input) => agent_write::move_path_in(&context, input, turn).await,
			Request::Delete(input) => agent_write::delete_in(&context, input, turn).await,
		}
	});
	let outcome = task
		.await
		.map_err(|_| "the write failed".to_string())?
		.map_err(|e| e.to_string())?;
	Ok(Output::json(
		serde_json::to_value(outcome).map_err(|e| e.to_string())?,
	))
}

#[derive(Deserialize)]
struct SearchContentArgs {
	query: String,
	#[serde(default)]
	kinds: Vec<TextKind>,
	#[serde(default)]
	max_results: Option<u32>,
	#[serde(default)]
	cursor: Option<String>,
}

async fn search_content(
	context: &Arc<CoreContext>,
	scope: &GrantScope,
	args: SearchContentArgs,
) -> Result<Output, String> {
	if args.query.len() > PATTERN_MAX_BYTES {
		return Err(format!(
			"the query is longer than {PATTERN_MAX_BYTES} bytes"
		));
	}
	// The grant's canonical path can differ from the source's registered
	// spelling by a firmlink, and the store answers only to the registered one.
	let Some((source_id, in_source)) =
		owning_source(context, scope.library_id, &scope.path).await?
	else {
		return Err(
			"this path is not inside a source Spacedrive tracks, so it has no text index; use grep"
				.to_string(),
		);
	};
	// Checked here so the refusal names this endpoint's tools.
	if !context
		.volume_index()
		.source_config(source_id)
		.is_some_and(|config| config.identify_content)
	{
		return Err(
			"this source does not read file contents, so it has no text index; use grep"
				.to_string(),
		);
	}

	let device_id = context
		.device_manager
		.device_id()
		.map_err(|_| "content search failed".to_string())?;
	let session = SessionContext::device_session(device_id, "Agent MCP".to_string())
		.with_library(scope.library_id);
	let found = ContentSearchQuery::from_input(ContentSearchInput {
		scope: SdPath::local(in_source),
		query: args.query,
		kinds: args.kinds,
		max_results: args.max_results,
		cursor: args.cursor,
	})
	.map_err(content_error)?
	.execute(context.clone(), session)
	.await
	.map_err(content_error)?;

	// Hit paths are relative to the searched path, and a file scope answers
	// with its own name. The index can lag the disk, so each hit is placed
	// by where it is now, like a search hit.
	let root = scope.root.clone();
	let base = scope.path.clone();
	let names: Vec<String> = found.hits.iter().map(|hit| hit.path.clone()).collect();
	let placed: Vec<Option<PathBuf>> = tokio::task::spawn_blocking(move || {
		let file_scope = std::fs::metadata(&base).is_ok_and(|meta| meta.is_file());
		names
			.iter()
			.map(|name| {
				let path = if file_scope {
					base.clone()
				} else {
					base.join(name)
				};
				std::fs::symlink_metadata(&path).ok()?;
				crate::ops::grants::scope::resolve(&root, &path)
					.ok()
					.map(|resolved| resolved.relative)
			})
			.collect()
	})
	.await
	.map_err(|_| "content search failed".to_string())?;

	let results: Vec<Value> = found
		.hits
		.into_iter()
		.zip(placed)
		.filter_map(|(hit, relative)| {
			let relative = relative?;
			let snippet: String = hit.snippet.iter().map(|part| part.text.as_str()).collect();
			let mut matched: Vec<&str> = hit
				.snippet
				.iter()
				.filter(|part| part.matched)
				.map(|part| part.text.as_str())
				.collect();
			matched.sort_unstable();
			matched.dedup();
			let mut result = json!({
				"path": paths::shown(&relative),
				"kind": hit.kind.as_str(),
				"score": hit.score,
				"snippet": snippet,
				"matched": matched,
				"text_truncated": hit.truncated,
			});
			if let Some(time) = hit.time {
				result["start_seconds"] = json!(time.start_seconds);
				result["end_seconds"] = json!(time.end_seconds);
			}
			Some(result)
		})
		.collect();
	Ok(Output::json(json!({
		"results": results,
		"next_cursor": found.next_cursor,
		"pending_files": found.pending_files,
		"note": "Content search reads Spacedrive's text index, which can lag the disk and covers only the first part of a large file. Confirm a hit with read.",
	})))
}

/// The op's refusals of the request reach the agent; anything else is
/// logged and answered generically, since it can name host paths outside
/// the grant.
fn content_error(error: QueryError) -> String {
	match error {
		QueryError::Validation { message, .. } | QueryError::InvalidInput(message) => message,
		other => {
			warn!(error = %other, "agent content search failed");
			"content search failed".to_string()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_tool_is_listed_and_parses() {
		let defs = definitions();
		let names: Vec<&str> = defs
			.as_array()
			.unwrap()
			.iter()
			.map(|t| t["name"].as_str().unwrap())
			.collect();
		let all: Vec<&str> = Tool::ALL.iter().map(|tool| tool.name()).collect();
		assert_eq!(names, all, "one definition per tool, in order");
		for tool in Tool::ALL {
			assert_eq!(Tool::parse(tool.name()), Some(tool));
		}
	}

	#[test]
	fn utf8_cut_mid_character_is_still_text() {
		let bytes = "héllo".as_bytes();
		let (text, encoding, covered) = encode(&bytes[..2]);
		assert_eq!((text.as_str(), encoding, covered), ("h", "utf-8", 1));
		let (_, encoding, _) = encode(&[0xff, 0xfe, 0x00]);
		assert_eq!(encoding, "base64");
	}

	#[test]
	fn read_refuses_a_cloud_placeholder() {
		let dir = tempfile::tempdir().unwrap();
		let root = std::fs::canonicalize(dir.path()).unwrap();
		std::fs::write(root.join("cloud.txt"), "not here").unwrap();
		let scope = GrantScope {
			grant_id: uuid::Uuid::new_v4(),
			library_id: uuid::Uuid::new_v4(),
			cap: GrantCap::Read,
			path: root.join("cloud.txt"),
			relative: PathBuf::from("cloud.txt"),
			root: root.clone(),
			expires_at: Utc::now(),
		};
		let args = || ReadArgs {
			offset: None,
			length: None,
			start_line: None,
			line_count: None,
			max_bytes: None,
		};
		let cancel = AtomicBool::new(false);
		assert!(read(&scope, args(), &cancel).is_ok());
		crate::ops::indexing::metadata::dataless_for_tests::mark(&scope.path);
		let refused = read(&scope, args(), &cancel).err().unwrap();
		assert!(refused.contains("cloud placeholder"), "{refused}");
	}

	fn lines(text: &[u8], start: u64, count: u64, cap: u64) -> Lines {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("f");
		std::fs::write(&path, text).unwrap();
		read_lines(
			File::open(&path).unwrap(),
			start,
			count,
			cap,
			&AtomicBool::new(false),
		)
		.unwrap()
	}

	#[test]
	fn a_line_longer_than_the_cap_continues_by_offset() {
		let long = vec![b'x'; 300_000];
		let read = lines(&long, 1, 10, 65_536);
		assert_eq!(read.content.len(), 65_536);
		assert!(read.truncated && !read.eof);
		assert_eq!(
			(read.returned, read.next_line, read.next_offset),
			(0, None, Some(65_536))
		);
	}

	#[test]
	fn skipped_lines_are_not_collected() {
		let mut text = vec![b'y'; 500_000];
		text.extend_from_slice(
			b"
second
third",
		);
		let read = lines(&text, 2, 1, 1024);
		assert_eq!(
			read.content,
			b"second
"
		);
		assert_eq!((read.first, read.returned, read.next_line), (2, 1, Some(3)));
		let read = lines(&text, 3, 5, 1024);
		assert_eq!(read.content, b"third");
		assert!(read.eof && read.next_line.is_none() && read.next_offset.is_none());
		assert_eq!(read.returned, 1, "a final line without a terminator counts");
	}

	#[test]
	fn a_cap_on_a_line_boundary_continues_by_line() {
		let read = lines(
			b"ab
cd
ef
",
			1,
			10,
			6,
		);
		assert_eq!(
			read.content,
			b"ab
cd
"
		);
		assert!(read.truncated);
		assert_eq!((read.next_line, read.next_offset), (Some(3), None));
		let read = lines(
			b"ab
", 5, 1, 6,
		);
		assert!(read.eof && read.content.is_empty() && read.returned == 0);
	}

	#[test]
	fn messages_lose_the_root() {
		assert_eq!(
			scrub("cannot read /srv/grant/a.txt", Path::new("/srv/grant")),
			"cannot read a.txt"
		);
	}
}
