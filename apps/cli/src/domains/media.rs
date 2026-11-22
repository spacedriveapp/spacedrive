//! Find clips by what was said, read a video's shots and transcript, and
//! export a rough cut, over the same ops the agent MCP tools use.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{Args, Subcommand, ValueEnum};
use sd_core::{
	domain::SdPath,
	ops::media::clips::{
		ClipSearchInput, ClipSearchOutput, ClipSnap, ExportClip, ExportMedia, FootageReadInput,
		FootageReadOutput, TimeInput, TimelineExportInput, TimelineExportOutput, TimelineFormat,
	},
};

use crate::{context::Context, util::prelude::*};

#[derive(Debug, Subcommand)]
pub enum MediaCmd {
	/// Clips found by what was said in them
	#[command(subcommand)]
	Clips(ClipsCmd),
	/// One video's shots and transcript
	#[command(subcommand)]
	Footage(FootageCmd),
	/// Rough cuts for editors
	#[command(subcommand)]
	Timeline(TimelineCmd),
}

#[derive(Debug, Subcommand)]
pub enum ClipsCmd {
	/// Find the moments where words were said
	Search(SearchArgs),
}

#[derive(Debug, Subcommand)]
pub enum FootageCmd {
	/// Print a video's shots, each with what was said in it
	Read(ReadArgs),
}

#[derive(Debug, Subcommand)]
pub enum TimelineCmd {
	/// Write FCPXML, EDL and OTIO from a list of clips
	Export(ExportArgs),
}

#[derive(Debug, Args)]
pub struct SearchArgs {
	/// Words that were said; quote a phrase, end a word with * for a prefix
	pub query: String,
	/// Directory or video to search. The current directory when absent
	#[arg(long)]
	pub path: Option<String>,
	/// Cut the whole shot instead of the speech plus padding
	#[arg(long, value_enum, default_value = "segment")]
	pub snap: Snap,
	/// Seconds of handle around the speech, 0 to 5
	#[arg(long)]
	pub pad: Option<f64>,
	#[arg(long)]
	pub limit: Option<u32>,
	#[arg(long)]
	pub cursor: Option<String>,
	/// Print the clips as JSON, which `sd media timeline export -` reads
	#[arg(long)]
	pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Snap {
	Segment,
	Shot,
}

#[derive(Debug, Args)]
pub struct ReadArgs {
	pub path: String,
	/// Seconds from the start to read from
	#[arg(long)]
	pub from: Option<f64>,
	/// Seconds from the start to read up to
	#[arg(long)]
	pub to: Option<f64>,
	#[arg(long)]
	pub json: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
	/// A JSON file of clips, or - for standard input. Takes the output of
	/// `sd media clips search --json`, or a list of
	/// {"sd_path" | "path", "in", "out", "label"?} objects
	pub clips: String,
	/// Project and file name
	#[arg(long)]
	pub name: String,
	/// Directory to write the files into
	#[arg(long)]
	pub out: String,
	/// Formats to write. Every format when absent
	#[arg(long, value_enum, value_delimiter = ',')]
	pub format: Vec<Format>,
	/// Reference the 720p proxies instead of the originals
	#[arg(long)]
	pub proxy: bool,
	#[arg(long)]
	pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Format {
	Fcpxml,
	Edl,
	Otio,
}

pub async fn run(ctx: &Context, command: MediaCmd) -> Result<()> {
	match command {
		MediaCmd::Clips(ClipsCmd::Search(args)) => search(ctx, args).await,
		MediaCmd::Footage(FootageCmd::Read(args)) => read(ctx, args).await,
		MediaCmd::Timeline(TimelineCmd::Export(args)) => export(ctx, args).await,
	}
}

/// A path as the user typed it: a URI, or a local path made absolute.
fn sd_path(ctx: &Context, text: &str) -> Result<SdPath> {
	if text.contains("://") {
		return Ok(SdPath::from_uri(text)?);
	}
	let path = PathBuf::from(text);
	if ctx.core.device().is_some() && !path.is_absolute() {
		anyhow::bail!("Use an absolute path when targeting another device");
	}
	let path = if path.is_absolute() {
		path
	} else {
		std::env::current_dir()?.join(path)
	};
	Ok(SdPath::Physical {
		device_slug: "local".into(),
		path,
	})
}

async fn search(ctx: &Context, args: SearchArgs) -> Result<()> {
	let scope = sd_path(ctx, args.path.as_deref().unwrap_or("."))?;
	let output: ClipSearchOutput = execute_query!(
		ctx,
		ClipSearchInput {
			scope,
			query: args.query,
			snap: match args.snap {
				Snap::Segment => ClipSnap::Segment,
				Snap::Shot => ClipSnap::Shot,
			},
			pad_seconds: args.pad,
			max_results: args.limit,
			cursor: args.cursor,
		}
	);
	if args.json {
		crate::util::output::print_json(&output);
		return Ok(());
	}
	for clip in &output.clips {
		println!(
			"{}  {} - {}  {}",
			clip.path, clip.in_point.timecode, clip.out_point.timecode, clip.transcript.text
		);
	}
	if output.clips.is_empty() {
		println!("No clips.");
	}
	if output.pending_files > 0 {
		println!(
			"{} files are not transcribed or indexed yet.",
			output.pending_files
		);
	}
	if let Some(cursor) = output.next_cursor {
		println!("More: --cursor {cursor}");
	}
	Ok(())
}

async fn read(ctx: &Context, args: ReadArgs) -> Result<()> {
	let seconds = |value: Option<f64>| {
		value.map(|seconds| TimeInput {
			frame: None,
			seconds: Some(seconds),
		})
	};
	let output: FootageReadOutput = execute_query!(
		ctx,
		FootageReadInput {
			path: sd_path(ctx, &args.path)?,
			from: seconds(args.from),
			to: seconds(args.to),
		}
	);
	if args.json {
		crate::util::output::print_json(&output);
		return Ok(());
	}
	println!(
		"{:.3} fps, {} long, starts at {}",
		output.rate.fps(),
		output.duration.timecode,
		output.start_timecode
	);
	for shot in &output.shots {
		println!(
			"{} - {}  {}",
			shot.in_point.timecode, shot.out_point.timecode, shot.text
		);
	}
	if output.shots.is_empty() {
		for segment in &output.segments {
			println!("{:>9.2}s  {}", segment.start, segment.text);
		}
	}
	if !output.missing.is_empty() {
		println!("Not made yet: {}", output.missing.join(", "));
	}
	if let Some(next) = output.next_from {
		println!("More: --from {next}");
	}
	Ok(())
}

/// One clip of the list `export` reads: a search result, or a hand-written
/// entry naming the file by `sd_path` or by `path`.
#[derive(serde::Deserialize)]
struct ListedClip {
	#[serde(default)]
	sd_path: Option<SdPath>,
	#[serde(default)]
	path: Option<String>,
	#[serde(rename = "in")]
	in_point: TimeInput,
	#[serde(rename = "out")]
	out_point: TimeInput,
	#[serde(default)]
	label: Option<String>,
}

fn parse_clips(ctx: &Context, text: &str) -> Result<Vec<ExportClip>> {
	let value: serde_json::Value = serde_json::from_str(text).context("the clips are not JSON")?;
	let list = match value.get("clips") {
		Some(clips) => clips.clone(),
		None => value,
	};
	let listed: Vec<ListedClip> =
		serde_json::from_value(list).context("expected a list of clips with in and out points")?;
	listed
		.into_iter()
		.enumerate()
		.map(|(index, clip)| {
			let path = match (clip.sd_path, clip.path) {
				(Some(sd_path), _) => sd_path,
				(None, Some(path)) => sd_path(ctx, &path)?,
				(None, None) => anyhow::bail!("clip {} names no file", index + 1),
			};
			Ok(ExportClip {
				path,
				in_point: clip.in_point,
				out_point: clip.out_point,
				label: clip.label,
			})
		})
		.collect()
}

async fn export(ctx: &Context, args: ExportArgs) -> Result<()> {
	let text = if args.clips == "-" {
		tokio::task::spawn_blocking(|| std::io::read_to_string(std::io::stdin())).await??
	} else {
		tokio::fs::read_to_string(&args.clips)
			.await
			.with_context(|| format!("reading {}", args.clips))?
	};
	let clips = parse_clips(ctx, &text)?;
	let output: TimelineExportOutput = execute_action!(
		ctx,
		TimelineExportInput {
			name: args.name,
			clips,
			formats: args
				.format
				.iter()
				.map(|format| match format {
					Format::Fcpxml => TimelineFormat::Fcpxml,
					Format::Edl => TimelineFormat::Edl,
					Format::Otio => TimelineFormat::Otio,
				})
				.collect(),
			media: if args.proxy {
				ExportMedia::Proxy
			} else {
				ExportMedia::Original
			},
			destination: Some(sd_path(ctx, &args.out)?),
		}
	);
	if args.json {
		crate::util::output::print_json(&output);
		return Ok(());
	}
	println!(
		"{} clips, {} at {:.3} fps",
		output.timeline.clips,
		output.timeline.duration.timecode,
		output.timeline.rate.fps()
	);
	for file in &output.files {
		match file.path.as_local_path() {
			Some(path) => println!("{}", path.display()),
			None => println!("{}", file.path),
		}
	}
	for warning in &output.warnings {
		println!("warning: {warning}");
	}
	Ok(())
}
