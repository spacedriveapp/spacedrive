//! The read tools on a bucket grant.
//!
//! A bucket is listed and read through its backend instead of the
//! filesystem, with the same arguments, limits and result shapes the local
//! tools use, so an agent cannot tell the two apart except where a bucket
//! has less to say: objects have no symlinks, creation or access times, and
//! no ignore files, so `respect_gitignore` has nothing to apply.
//!
//! Index-backed tools answer from a source's store, which a bucket source
//! does not have yet; they say so and point at `grep`.

use std::{
	collections::VecDeque,
	path::{Path, PathBuf},
	sync::{atomic::AtomicBool, Arc},
	time::SystemTime,
};

use serde_json::{json, Value};

use super::*;
use crate::{
	ops::{
		grants::bucket::{self as grant_bucket, BucketUrl},
		indexing::state::EntryKind,
		search::grep::{scan::Page, GrepSkipReason},
	},
	volume::{RawDirEntry, VolumeBackend, VolumeError},
};

/// The largest object a line-mode read loads whole.
const LINE_READ_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Run a read tool on a bucket scope. `None` for a tool the bucket does not
/// handle itself: the skill text and the writes, which reach the bucket
/// through the agent write operations.
pub(super) async fn call(
	context: &Arc<CoreContext>,
	tool: Tool,
	scope: &GrantScope,
	url: BucketUrl,
	arguments: &Value,
	cancel: &AtomicBool,
) -> Option<Result<Output, String>> {
	if matches!(
		tool,
		Tool::GetSkill | Tool::WriteFile | Tool::MakeDir | Tool::Move | Tool::Delete
	) {
		return None;
	}
	let backend = match grant_bucket::backend(context, &url).await {
		Ok(backend) => backend,
		Err(error) => return Some(Err(error.to_string())),
	};
	let backend = backend.as_ref();
	Some(match tool {
		Tool::ListDir => match args(arguments.clone()) {
			Ok(args) => list_dir(backend, scope, &url, args).await,
			Err(error) => Err(error),
		},
		Tool::Stat => stat(context, backend, scope, &url).await,
		Tool::Read => match args(arguments.clone()) {
			Ok(args) => read(backend, scope, &url, args, cancel).await,
			Err(error) => Err(error),
		},
		Tool::Glob => match args(arguments.clone()) {
			Ok(args) => glob(backend, scope, &url, args, cancel).await,
			Err(error) => Err(error),
		},
		Tool::Grep => match args(arguments.clone()) {
			Ok(args) => grep(backend, scope, &url, args, cancel).await,
			Err(error) => Err(error),
		},
		_ => Err(format!(
			"{} reads Spacedrive's index, which bucket sources do not have yet; use grep or glob",
			tool.name()
		)),
	})
}

fn message(error: VolumeError) -> String {
	match error {
		VolumeError::Io(error) => io_message(&error),
		other => other.to_string(),
	}
}

fn modified(at: Option<SystemTime>) -> Option<String> {
	at.map(|at| DateTime::<Utc>::from(at).to_rfc3339())
}

fn kind_name(kind: EntryKind) -> &'static str {
	match kind {
		EntryKind::Directory => "dir",
		_ => "file",
	}
}

/// What the key is: an object, a prefix with objects under it, or nothing.
async fn object_size(backend: &dyn VolumeBackend, url: &BucketUrl) -> Result<Option<u64>, String> {
	if url.key.is_empty() {
		return Ok(None);
	}
	match backend.metadata(Path::new(&url.key)).await {
		Ok(meta) if meta.kind != EntryKind::Directory => Ok(Some(meta.size)),
		Ok(_) => Ok(None),
		Err(VolumeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(error) => Err(message(error)),
	}
}

async fn list_dir(
	backend: &dyn VolumeBackend,
	scope: &GrantScope,
	url: &BucketUrl,
	args: ListArgs,
) -> Result<Output, String> {
	let entries = grant_bucket::list(backend, url).await.map_err(message)?;
	if entries.is_empty() && !url.key.is_empty() {
		return Err(match object_size(backend, url).await? {
			Some(_) => "not a directory".to_string(),
			None => "no such file or directory".to_string(),
		});
	}
	let offset = page_offset(args.cursor.as_deref())?;
	let limit = page_limit(args.limit, PAGE_DEFAULT, PAGE_MAX);
	let page: Vec<Value> = entries
		.iter()
		.skip(offset)
		.take(limit)
		.map(|entry| {
			json!({
				"name": entry.name,
				"path": paths::joined(&scope.relative, &entry.name),
				"type": kind_name(entry.kind),
				"size": entry.size,
				"modified": modified(entry.modified),
			})
		})
		.collect();
	let next = offset + page.len();
	Ok(Output::json(json!({
		"path": paths::shown(&scope.relative),
		"entries": page,
		"total": entries.len(),
		"next_cursor": (next < entries.len()).then(|| next.to_string()),
	})))
}

async fn stat(
	context: &Arc<CoreContext>,
	backend: &dyn VolumeBackend,
	scope: &GrantScope,
	url: &BucketUrl,
) -> Result<Output, String> {
	let state = agent_write::state_of(
		context,
		&scope.root,
		&scope.path,
		Arc::new(AtomicBool::new(false)),
	)
	.await
	.map_err(|e| e.to_string())?;
	let Some(kind) = state.kind else {
		return Err("no such file or directory".to_string());
	};
	let file_modified = match kind {
		agent_write::PathKind::File => backend
			.metadata(Path::new(&url.key))
			.await
			.ok()
			.and_then(|meta| modified(meta.modified)),
		_ => None,
	};
	Ok(Output::json(json!({
		"path": paths::shown(&scope.relative),
		"type": match kind {
			agent_write::PathKind::Directory => "dir",
			_ => "file",
		},
		"size": state.size,
		"modified": file_modified,
		"created": Value::Null,
		"accessed": Value::Null,
		"readonly": false,
		"content_hash": state.content_hash,
		"revision": state.revision,
	})))
}

async fn read(
	backend: &dyn VolumeBackend,
	scope: &GrantScope,
	url: &BucketUrl,
	args: ReadArgs,
	cancel: &AtomicBool,
) -> Result<Output, String> {
	let cap = args
		.max_bytes
		.unwrap_or(READ_DEFAULT_BYTES)
		.clamp(1, READ_MAX_BYTES);
	if args.start_line.is_some() && (args.offset.is_some() || args.length.is_some()) {
		return Err("use either offset and length, or start_line and line_count".to_string());
	}
	let size = match object_size(backend, url).await? {
		Some(size) => size,
		None if grant_bucket::list(backend, url)
			.await
			.is_ok_and(|entries| !entries.is_empty()) =>
		{
			return Err("not a regular file".to_string())
		}
		None => return Err("no such file or directory".to_string()),
	};
	let path = paths::shown(&scope.relative);
	let key = Path::new(&url.key);

	if let Some(start_line) = args.start_line {
		if size > LINE_READ_MAX_BYTES {
			return Err(format!(
				"line reads load the whole object, and this one is over {} MiB; use offset and length",
				LINE_READ_MAX_BYTES / (1024 * 1024)
			));
		}
		let bytes = backend.read(key).await.map_err(message)?;
		let count = args.line_count.unwrap_or(READ_DEFAULT_LINES).max(1);
		let lines = read_lines(
			std::io::Cursor::new(bytes),
			start_line.max(1),
			count,
			cap,
			cancel,
		)?;
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
	let end = offset.saturating_add(want).min(size);
	let bytes = if offset >= end {
		Vec::new()
	} else {
		backend
			.read_range(key, offset..end)
			.await
			.map_err(message)?
			.to_vec()
	};
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

/// Every key under `url`, depth first in path order, relative to it. Stops
/// after `GLOB_VISIT_MAX` entries and says so.
async fn walk(
	backend: &dyn VolumeBackend,
	url: &BucketUrl,
	include_hidden: bool,
	cancel: &AtomicBool,
) -> Result<(Vec<(PathBuf, RawDirEntry)>, bool), String> {
	let mut found = Vec::new();
	let mut pending = VecDeque::from([PathBuf::new()]);
	while let Some(dir) = pending.pop_front() {
		if cancel.load(std::sync::atomic::Ordering::Relaxed) {
			return Err("cancelled".to_string());
		}
		let here = url.join(&paths::shown(&dir));
		for entry in grant_bucket::list(backend, &here).await.map_err(message)? {
			if !include_hidden && entry.name.starts_with('.') {
				continue;
			}
			let relative = dir.join(&entry.name);
			if entry.kind == EntryKind::Directory {
				pending.push_back(relative.clone());
			}
			found.push((relative, entry));
			if found.len() as u64 >= GLOB_VISIT_MAX {
				found.sort_by(|a, b| a.0.cmp(&b.0));
				return Ok((found, true));
			}
		}
	}
	found.sort_by(|a, b| a.0.cmp(&b.0));
	Ok((found, false))
}

async fn glob(
	backend: &dyn VolumeBackend,
	scope: &GrantScope,
	url: &BucketUrl,
	args: GlobArgs,
	cancel: &AtomicBool,
) -> Result<Output, String> {
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
	let offset = page_offset(args.cursor.as_deref())?;
	let limit = page_limit(args.limit, PAGE_DEFAULT, PAGE_MAX);
	let (entries, stopped) = walk(backend, url, args.include_hidden, cancel).await?;
	if entries.is_empty() && object_size(backend, url).await?.is_some() {
		return Err("not a directory".to_string());
	}
	let hits: Vec<_> = entries
		.iter()
		.filter(|(relative, _)| matcher.is_match(relative))
		.collect();
	let matches: Vec<Value> = hits
		.iter()
		.skip(offset)
		.take(limit)
		.map(|(relative, entry)| {
			json!({
				"path": paths::joined(&scope.relative, &paths::shown(relative)),
				"type": kind_name(entry.kind),
			})
		})
		.collect();
	let next = offset + matches.len();
	Ok(Output::json(json!({
		"matches": matches,
		"next_cursor": (next < hits.len()).then(|| next.to_string()),
		"visit_limit_reached": stopped,
		"entries_visited": entries.len(),
	})))
}

fn globs(patterns: &[String]) -> Result<Option<globset::GlobSet>, String> {
	if patterns.is_empty() {
		return Ok(None);
	}
	let mut set = globset::GlobSetBuilder::new();
	for pattern in patterns {
		set.add(
			globset::Glob::new(pattern)
				.map_err(|e| format!("invalid glob '{pattern}': {}", e.kind()))?,
		);
	}
	set.build().map(Some).map_err(|e| e.to_string())
}

/// A glob without a `/` matches a name at any depth, as it does in the local
/// grep's ignore-style overrides.
fn glob_hit(set: &globset::GlobSet, relative: &Path) -> bool {
	set.is_match(relative) || relative.file_name().is_some_and(|name| set.is_match(name))
}

async fn grep(
	backend: &dyn VolumeBackend,
	scope: &GrantScope,
	url: &BucketUrl,
	args: GrepArgs,
	cancel: &AtomicBool,
) -> Result<Output, String> {
	if args.pattern.len() > PATTERN_MAX_BYTES {
		return Err(format!(
			"the pattern is longer than {PATTERN_MAX_BYTES} bytes"
		));
	}
	let include = globs(&args.include)?;
	let exclude = globs(&args.exclude)?;
	let options = GrepOptions {
		pattern: args.pattern,
		literal: args.literal,
		case_insensitive: args.case_insensitive,
		include: Vec::new(),
		exclude: Vec::new(),
		respect_gitignore: false,
		include_hidden: args.include_hidden,
		context_lines: args.context_lines,
		max_results: Some(args.max_results.unwrap_or(100).clamp(1, GREP_PAGE_MAX)),
		cursor: args.cursor,
		max_file_bytes: None,
	};
	let mut page = Page::new(&options, cancel).map_err(|e| e.to_string())?;

	// A file scope searches that one object and names matches by its name.
	let (files, base, stopped): (Vec<(PathBuf, String, u64)>, &Path, bool) =
		match object_size(backend, url).await? {
			Some(size) => (
				vec![(PathBuf::new(), url.key.clone(), size)],
				scope.relative.parent().unwrap_or(Path::new("")),
				false,
			),
			None => {
				let (entries, stopped) = walk(backend, url, args.include_hidden, cancel).await?;
				let files = entries
					.into_iter()
					.filter(|(_, entry)| entry.kind != EntryKind::Directory)
					.filter(|(relative, _)| {
						include.as_ref().is_none_or(|set| glob_hit(set, relative))
							&& !exclude.as_ref().is_some_and(|set| glob_hit(set, relative))
					})
					.map(|(relative, entry)| {
						let key = url.join(&paths::shown(&relative)).key;
						(relative, key, entry.size)
					})
					.collect();
				(files, scope.relative.as_path(), stopped)
			}
		};
	for (relative, key, size) in files {
		let shown = if relative.as_os_str().is_empty() {
			key.rsplit('/').next().unwrap_or_default().to_string()
		} else {
			paths::shown(&relative)
		};
		if !page.wants(&relative, size, &shown) {
			continue;
		}
		let bytes = match backend.read(Path::new(&key)).await {
			Ok(bytes) => bytes,
			Err(error) => {
				tracing::debug!(%key, %error, "grep skipped an unreadable object");
				continue;
			}
		};
		if bytes.len() as u64 > page.max_file_bytes() {
			page.skip(&shown, GrepSkipReason::TooLarge);
			continue;
		}
		if page
			.search(&relative, &shown, &bytes)
			.map_err(|e| e.to_string())?
		{
			break;
		}
	}
	let out = page.finish();
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
		"truncated": out.truncated || stopped,
		"next_cursor": out.next_cursor,
		"visit_limit_reached": stopped,
	})))
}
