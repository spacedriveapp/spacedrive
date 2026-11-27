//! # Agent MCP footage tools
//!
//! `search_clips`, `read_footage` and `export_timeline` over the
//! `media.clips` ops. Every path an agent sends or reads is relative to the
//! grant root, so a clip of a file outside the granted directory is never
//! returned. The timelines `export_timeline` writes are the one place host
//! paths reach the agent: an editor needs absolute file URLs, and they name
//! only files inside the grant, or the library's proxy of one.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use serde::Deserialize;
use serde_json::{json, Value};

use super::{
	limits::PATTERN_MAX_BYTES,
	paths,
	tools::{content_error, io_message, place, session, text_index_path, Output},
};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::query::LibraryQuery,
	ops::{
		agent_write::{self, WriteFileInput},
		grants::GrantScope,
		indexing::metadata::{refuse_dataless, DATALESS_REASON},
		media::clips::{
			render_timeline, ClipSearchInput, ClipSearchQuery, ClipSnap, ExportClip, ExportMedia,
			FootageReadInput, FootageReadQuery, TimeInput, TimelineExportInput, TimelineFormat,
			DEFAULT_PAD_SECONDS, MAX_EXPORT_CLIPS, MAX_PAD_SECONDS,
		},
	},
};

/// The tools' entries in `tools/list`, in `Tool::ALL` order.
pub fn definitions(path: &dyn Fn(&str) -> Value) -> Vec<Value> {
	let time = |what: &str| {
		json!({
			"type": "object",
			"description": format!("{what}: {{\"frame\": n}} at the file's frame rate, or {{\"seconds\": x}}, which rounds to the nearest frame. A frame wins when both are given, so an in or out point from search_clips or read_footage can be passed as it is."),
			"properties": {
				"frame": { "type": "integer", "minimum": 0 },
				"seconds": { "type": "number", "minimum": 0 },
			},
		})
	};
	let annotations = |title: &str, read_only: bool| {
		json!({
			"title": title,
			"readOnlyHint": read_only,
			"destructiveHint": false,
			"idempotentHint": read_only,
			"openWorldHint": false,
		})
	};
	vec![
		json!({
			"name": "search_clips",
			"title": "Find clips by what was said",
			"description": "Searches the transcripts of the videos under 'path' and returns one clip per moment the words were said: the video, its in and out points (whole frames at the file's frame rate, with seconds and timecode; out is the first frame after the clip), the shot it sits in, and the transcript text of that range. Every word must appear in one transcript segment; text in double quotes is a phrase and a word ending in '*' matches as a prefix. snap 'segment' cuts the speech plus pad_seconds of handle, kept inside its shot; snap 'shot' returns the whole shot. Neighbouring hits in one shot merge into one clip. Clips are ordered by relevance; sort by path and in.frame for story order. It answers from the transcript index, so a video not transcribed yet is missing and pending_files counts files waiting. Pass the clips you keep to export_timeline.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"query": { "type": "string", "maxLength": PATTERN_MAX_BYTES, "description": "Words that were said, such as 'train' or '\"missed the train\"'." },
					"path": path("The directory or video to search"),
					"snap": { "type": "string", "enum": ["segment", "shot"], "default": "segment" },
					"pad_seconds": { "type": "number", "minimum": 0, "maximum": MAX_PAD_SECONDS, "default": DEFAULT_PAD_SECONDS },
					"max_results": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
					"cursor": { "type": "string", "description": "The next_cursor of the previous page, to continue from there." },
				},
				"required": ["query"],
				"additionalProperties": false,
			},
			"annotations": annotations("Find clips by what was said", true),
		}),
		json!({
			"name": "read_footage",
			"title": "Read a video's shots and transcript",
			"description": "Reads one video's shot list and transcript: each shot with its in and out points and what was said in it, and each transcript segment with its start and end in seconds and its shot. Use it to pick and trim clips by reading. from and to bound the answer for a long video; an answer holds at most 500 segments and next_from continues after the last. missing names what has not been made yet: transcript, shots or proxy.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"path": path("The video file"),
					"from": time("Read from here"),
					"to": time("Read up to here"),
				},
				"required": ["path"],
				"additionalProperties": false,
			},
			"annotations": annotations("Read a video's shots and transcript", true),
		}),
		json!({
			"name": "export_timeline",
			"title": "Export a rough cut",
			"description": "Builds a rough cut from clips in the order given and writes it as FCPXML (Final Cut Pro, Resolve), a CMX 3600 EDL (Resolve, Premiere Pro) and OpenTimelineIO, each referencing the original files, or their 720p proxies with media 'proxy'. Clips sit end to end with their sound. A clip from search_clips can be passed as it is. With write_to, the files are written into that directory of the grant like write_file without expect (a taken name gets a conflict suffix, each file has an undo id), which needs a read-write grant; without it, the documents come back in the response. The files hold absolute paths of the videos, because editors need them. warnings say what an editor will do differently, such as conforming a clip at another frame rate.",
			"inputSchema": {
				"type": "object",
				"properties": {
					"name": { "type": "string", "minLength": 1, "maxLength": 120, "description": "The project and file name." },
					"clips": {
						"type": "array",
						"minItems": 1,
						"maxItems": MAX_EXPORT_CLIPS,
						"items": {
							"type": "object",
							"properties": {
								"path": path("The video"),
								"in": time("First frame of the clip"),
								"out": time("First frame after the clip"),
								"label": { "type": "string", "description": "The clip's name on the timeline. The file name when absent." },
							},
							"required": ["path", "in", "out"],
						},
					},
					"formats": {
						"type": "array",
						"items": { "type": "string", "enum": ["fcpxml", "edl", "otio"] },
						"description": "Every format when empty or absent.",
					},
					"media": { "type": "string", "enum": ["original", "proxy"], "default": "original" },
					"write_to": path("The directory to write the files into"),
				},
				"required": ["name", "clips"],
				"additionalProperties": false,
			},
			"annotations": annotations("Export a rough cut", false),
		}),
	]
}

#[derive(Deserialize)]
pub struct SearchClipsArgs {
	query: String,
	#[serde(default)]
	snap: ClipSnap,
	#[serde(default)]
	pad_seconds: Option<f64>,
	#[serde(default)]
	max_results: Option<u32>,
	#[serde(default)]
	cursor: Option<String>,
}

pub async fn search_clips(
	context: &Arc<CoreContext>,
	scope: &GrantScope,
	args: SearchClipsArgs,
) -> Result<Output, String> {
	if args.query.len() > PATTERN_MAX_BYTES {
		return Err(format!(
			"the query is longer than {PATTERN_MAX_BYTES} bytes"
		));
	}
	let in_source = text_index_path(context, scope).await?;
	let found = ClipSearchQuery::from_input(ClipSearchInput {
		scope: SdPath::local(in_source),
		query: args.query,
		snap: args.snap,
		pad_seconds: args.pad_seconds,
		max_results: args.max_results,
		cursor: args.cursor,
	})
	.map_err(content_error)?
	.execute(context.clone(), session(context, scope)?)
	.await
	.map_err(content_error)?;
	let names = found.clips.iter().map(|clip| clip.path.clone()).collect();
	let placed = place(scope, names).await?;
	let clips: Vec<Value> = found
		.clips
		.into_iter()
		.zip(placed)
		.filter_map(|(clip, relative)| {
			let mut value = serde_json::to_value(&clip).ok()?;
			value["path"] = json!(paths::shown(&relative?));
			value.as_object_mut()?.remove("sd_path");
			Some(value)
		})
		.collect();
	Ok(Output::json(json!({
		"clips": clips,
		"next_cursor": found.next_cursor,
		"pending_files": found.pending_files,
		"note": "Clips come from the transcript index, which can lag new footage. pending_files counts files not read yet.",
	})))
}

#[derive(Deserialize)]
pub struct ReadFootageArgs {
	#[serde(default)]
	from: Option<TimeInput>,
	#[serde(default)]
	to: Option<TimeInput>,
}

pub async fn read_footage(
	context: &Arc<CoreContext>,
	scope: &GrantScope,
	args: ReadFootageArgs,
) -> Result<Output, String> {
	video_file(&scope.root, &scope.relative).await?;
	let read = FootageReadQuery::from_input(FootageReadInput {
		path: SdPath::local(scope.path.clone()),
		from: args.from,
		to: args.to,
	})
	.map_err(content_error)?
	.execute(context.clone(), session(context, scope)?)
	.await
	.map_err(content_error)?;
	let mut value = serde_json::to_value(&read).map_err(|e| e.to_string())?;
	value["path"] = json!(paths::shown(&scope.relative));
	Ok(Output::json(value))
}

#[derive(Deserialize)]
pub struct ExportClipArgs {
	path: String,
	#[serde(rename = "in")]
	in_point: TimeInput,
	#[serde(rename = "out")]
	out_point: TimeInput,
	#[serde(default)]
	label: Option<String>,
}

#[derive(Deserialize)]
pub struct ExportTimelineArgs {
	name: String,
	clips: Vec<ExportClipArgs>,
	#[serde(default)]
	formats: Vec<TimelineFormat>,
	#[serde(default)]
	media: ExportMedia,
	#[serde(default)]
	write_to: Option<String>,
}

pub async fn export_timeline(
	context: &Arc<CoreContext>,
	token: &str,
	scope: &GrantScope,
	args: ExportTimelineArgs,
) -> Result<Output, String> {
	if args.clips.is_empty() || args.clips.len() > MAX_EXPORT_CLIPS {
		return Err(format!("a timeline takes 1 to {MAX_EXPORT_CLIPS} clips"));
	}
	if let Some(write_to) = &args.write_to {
		paths::check(write_to)?;
	}
	let mut clips = Vec::with_capacity(args.clips.len());
	for (position, clip) in args.clips.into_iter().enumerate() {
		paths::check(&clip.path).map_err(|reason| format!("clip {}: {reason}", position + 1))?;
		let root = scope.root.clone();
		let requested = PathBuf::from(&clip.path);
		let resolved = tokio::task::spawn_blocking(move || {
			crate::ops::grants::scope::resolve(&root, &requested)
		})
		.await
		.map_err(|_| "export failed".to_string())?
		.map_err(|error| format!("clip {}: {error}", position + 1))?;
		video_file(&resolved.root, &resolved.relative)
			.await
			.map_err(|reason| format!("clip {} ({}): {reason}", position + 1, clip.path))?;
		clips.push(ExportClip {
			path: SdPath::local(resolved.path),
			in_point: clip.in_point,
			out_point: clip.out_point,
			label: clip.label,
		});
	}
	let library = context
		.libraries()
		.await
		.get_library(scope.library_id)
		.await
		.ok_or_else(|| "the grant's library is not open".to_string())?;
	let input = TimelineExportInput {
		name: args.name,
		clips,
		formats: args.formats,
		media: args.media,
		destination: None,
	};
	let mut output = render_timeline(context, &library, &input).await?;
	let Some(write_to) = args.write_to else {
		return Ok(Output::json(
			serde_json::to_value(&output).map_err(|e| e.to_string())?,
		));
	};
	let mut files = Vec::new();
	for document in std::mem::take(&mut output.documents) {
		let outcome = agent_write::write_file(
			context,
			WriteFileInput {
				token: token.to_string(),
				path: Path::new(&write_to).join(&document.file_name),
				content: document.content,
				encoding: None,
				offset: None,
				expect: None,
			},
		)
		.await
		.map_err(|error| error.to_string())?;
		files.push(json!({
			"format": document.format,
			"path": outcome.state.path,
			"conflict": outcome.conflict,
			"undo_id": outcome.undo,
		}));
	}
	Ok(Output::json(json!({
		"timeline": output.timeline,
		"files": files,
		"warnings": output.warnings,
	})))
}

/// Refuses anything but a regular file beneath the grant root, described
/// through a handle there, and a cloud placeholder, which would be downloaded
/// to be read.
async fn video_file(root: &Path, relative: &Path) -> Result<(), String> {
	let (owned_root, owned_relative) = (root.to_path_buf(), relative.to_path_buf());
	let meta = tokio::task::spawn_blocking(move || {
		crate::common::contain::stat_beneath(&owned_root, &owned_relative)
	})
	.await
	.map_err(|_| "the call failed".to_string())?
	.map_err(|e| io_message(&e))?;
	if meta.kind != crate::common::contain::Kind::File {
		return Err("this takes a video file, not a directory".to_string());
	}
	if refuse_dataless(&root.join(relative)).await.is_err() {
		return Err(format!(
			"the file is a cloud placeholder ({DATALESS_REASON}); it is not read so it is not downloaded"
		));
	}
	Ok(())
}
