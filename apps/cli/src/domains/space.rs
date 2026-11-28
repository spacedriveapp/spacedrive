//! `sd space`: create, list, show, update and delete spaces, and read and
//! restore the versions of a space that keeps them.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use uuid::Uuid;

use sd_core::{
	domain::{Space, SpaceKind},
	ops::spaces::{
		SpaceCreateInput, SpaceCreateOutput, SpaceDeleteInput, SpaceDeleteOutput, SpaceExportInput,
		SpaceExportOutput, SpaceGetQueryInput, SpaceGetQueryOutput, SpaceHistoryInput,
		SpaceHistoryOutput, SpaceHistoryPurgeInput, SpaceHistoryPurgeOutput, SpaceImportInput,
		SpaceImportOutput, SpaceRestoreInput, SpaceRestoreOutput, SpaceSnapshot,
		SpaceSnapshotCreateInput, SpaceSnapshotCreateOutput, SpaceSnapshotDeleteInput,
		SpaceSnapshotDeleteOutput, SpaceSnapshotListInput, SpaceSnapshotListOutput,
		SpaceUpdateInput, SpaceUpdateOutput, SpacesListQueryInput, SpacesListQueryOutput,
	},
};

use crate::context::Context;
use crate::util::prelude::*;

#[derive(Subcommand, Debug)]
pub enum SpaceCmd {
	/// Make a folder a space, or create an empty workspace space
	Create(SpaceCreateArgs),
	/// List the spaces in the library
	List {
		/// Only the spaces in this source
		#[arg(long)]
		source: Option<Uuid>,
	},
	/// Show one space
	Show { id: Uuid },
	/// Rename a space or turn its content indexing or versioning on or off
	Update(SpaceUpdateArgs),
	/// Stop treating a folder as a space; its files stay
	Delete { id: Uuid },
	/// List the stored versions of a space, a folder in it, or one file
	History(SpaceHistoryArgs),
	/// Write an earlier version, or a snapshot, back into the space
	Restore(SpaceRestoreArgs),
	/// Name, list and remove snapshots of a space
	#[command(subcommand)]
	Snapshot(SnapshotCmd),
	/// Delete every stored version and snapshot of a space; its files stay
	PurgeHistory { id: Uuid },
	/// Write a space, its change log and its versions to one .sdspace file
	Export {
		id: Uuid,
		/// The file to write; it must not exist
		path: PathBuf,
		/// Only the newest version of each file, with no snapshots
		#[arg(long)]
		current: bool,
	},
	/// Create a space from a .sdspace file, with its files, change log and versions
	Import {
		/// The .sdspace file
		path: PathBuf,
		/// The folder the space's files go in; it must not exist or must be empty
		destination: PathBuf,
		/// The new space's name; the exported name by default
		#[arg(long)]
		name: Option<String>,
	},
}

#[derive(Args, Debug)]
pub struct SpaceHistoryArgs {
	pub id: Uuid,
	/// A file or folder, relative to the space root
	#[arg(long)]
	pub path: Option<String>,
	/// Versions older than this id
	#[arg(long)]
	pub before: Option<i64>,
	#[arg(long, default_value_t = 50)]
	pub limit: u32,
}

#[derive(Args, Debug)]
pub struct SpaceRestoreArgs {
	pub id: Uuid,
	/// A version id from `sd space history`
	#[arg(
		long,
		conflicts_with = "snapshot",
		required_unless_present = "snapshot"
	)]
	pub version: Option<i64>,
	/// A snapshot name or id
	#[arg(long)]
	pub snapshot: Option<String>,
	/// With --version, where to write it; with --snapshot, the file or
	/// folder to restore. Relative to the space root
	#[arg(long)]
	pub path: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SnapshotCmd {
	/// Pin the current state of a space under a name
	Create { id: Uuid, name: String },
	/// List a space's snapshots
	List { id: Uuid },
	/// Remove a snapshot; its versions then age out
	Delete { id: Uuid, snapshot: Uuid },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum KindArg {
	Folder,
	Workspace,
}

#[derive(Args, Debug)]
pub struct SpaceCreateArgs {
	/// The folder. Required for a folder space; for a workspace, the empty
	/// folder to create instead of one under the data directory
	pub path: Option<PathBuf>,
	#[arg(long, value_enum, default_value = "folder")]
	pub kind: KindArg,
	#[arg(long)]
	pub name: Option<String>,
	/// Do not hash or read file contents inside the space
	#[arg(long)]
	pub no_content_indexing: bool,
}

#[derive(Args, Debug)]
pub struct SpaceUpdateArgs {
	pub id: Uuid,
	#[arg(long)]
	pub name: Option<String>,
	/// Turn content indexing on or off
	#[arg(long)]
	pub content_indexing: Option<bool>,
	/// Keep every change's bytes as a version. Turning it off deletes the
	/// space's versions and snapshots
	#[arg(long)]
	pub versioning: Option<bool>,
}

pub async fn run(ctx: &Context, cmd: SpaceCmd) -> Result<()> {
	match cmd {
		SpaceCmd::Create(args) => {
			// A bucket URL names a prefix, not a path relative to here.
			let path = match args.path {
				Some(path) if path.to_string_lossy().contains("://") => Some(path),
				Some(path) => Some(std::path::absolute(path)?),
				None => None,
			};
			let input = SpaceCreateInput {
				name: args.name,
				kind: match args.kind {
					KindArg::Folder => SpaceKind::Folder,
					KindArg::Workspace => SpaceKind::Workspace,
				},
				path,
				content_indexing: args.no_content_indexing.then_some(false),
			};
			let out: SpaceCreateOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceCreateOutput| print_space(&o.space));
		}
		SpaceCmd::List { source } => {
			let out: SpacesListQueryOutput =
				execute_query!(ctx, SpacesListQueryInput { source_id: source });
			print_output!(ctx, &out, |o: &SpacesListQueryOutput| {
				if o.spaces.is_empty() {
					println!("No spaces");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["ID", "Name", "Kind", "Content", "Path"]);
				for space in &o.spaces {
					table.add_row(vec![
						space.id.to_string(),
						space.name.clone(),
						kind(space).to_string(),
						if space.content_indexing { "on" } else { "off" }.to_string(),
						location(space),
					]);
				}
				println!("{table}");
			});
		}
		SpaceCmd::Show { id } => {
			let out: SpaceGetQueryOutput = execute_query!(ctx, SpaceGetQueryInput { id });
			print_output!(ctx, &out, |o: &SpaceGetQueryOutput| print_space(&o.space));
		}
		SpaceCmd::Update(args) => {
			let input = SpaceUpdateInput {
				id: args.id,
				name: args.name,
				content_indexing: args.content_indexing,
				versioning: args.versioning,
			};
			let out: SpaceUpdateOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceUpdateOutput| print_space(&o.space));
		}
		SpaceCmd::Delete { id } => {
			let out: SpaceDeleteOutput = execute_action!(ctx, SpaceDeleteInput { id });
			print_output!(ctx, &out, |o: &SpaceDeleteOutput| {
				println!("Deleted space {}; its files are untouched", o.id);
			});
		}
		SpaceCmd::History(args) => {
			let input = SpaceHistoryInput {
				id: args.id,
				path: args.path,
				before: args.before,
				limit: Some(args.limit),
			};
			let out: SpaceHistoryOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceHistoryOutput| {
				if o.versions.is_empty() {
					if o.versioning {
						println!("No versions");
					} else {
						println!("This space does not keep versions; turn it on with sd space update <id> --versioning true");
					}
					return;
				}
				if !o.versioning {
					println!("Versioning is off; these versions were kept from before and age out after {} days", o.retention_days);
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["Version", "When", "Path", "Size", "By", "How"]);
				for version in &o.versions {
					table.add_row(vec![
						version.id.to_string(),
						version.at.clone(),
						version.path.clone(),
						if version.deleted {
							"deleted".to_string()
						} else {
							version
								.size
								.map(|size| size.to_string())
								.unwrap_or_default()
						},
						version.actor.label.clone(),
						version.origin.clone(),
					]);
				}
				println!("{table}");
				println!("Versions are kept {} days unless a snapshot pins them. A change overwritten before Spacedrive read it has no version.", o.retention_days);
			});
		}
		SpaceCmd::Restore(args) => {
			let input = SpaceRestoreInput {
				id: args.id,
				version: args.version,
				snapshot: args.snapshot,
				path: args.path,
			};
			let out: SpaceRestoreOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceRestoreOutput| {
				for file in &o.restored {
					let undo = file
						.undo
						.map(|id| format!(" (undo with sd file undo {id})"))
						.unwrap_or_default();
					if file.conflict {
						println!("Restored {} beside it as {}{undo}", file.path, file.landed);
					} else {
						println!("Restored {}{undo}", file.path);
					}
				}
				if o.unchanged > 0 {
					println!("{} already matched", o.unchanged);
				}
				for failure in &o.failed {
					println!("Could not restore {}: {}", failure.path, failure.error);
				}
			});
		}
		SpaceCmd::Snapshot(SnapshotCmd::Create { id, name }) => {
			let out: SpaceSnapshotCreateOutput =
				execute_action!(ctx, SpaceSnapshotCreateInput { id, name });
			print_output!(ctx, &out, |o: &SpaceSnapshotCreateOutput| print_snapshots(
				std::slice::from_ref(&o.snapshot)
			));
		}
		SpaceCmd::Snapshot(SnapshotCmd::List { id }) => {
			let out: SpaceSnapshotListOutput = execute_query!(ctx, SpaceSnapshotListInput { id });
			print_output!(ctx, &out, |o: &SpaceSnapshotListOutput| print_snapshots(
				&o.snapshots
			));
		}
		SpaceCmd::Snapshot(SnapshotCmd::Delete { id, snapshot }) => {
			let out: SpaceSnapshotDeleteOutput =
				execute_action!(ctx, SpaceSnapshotDeleteInput { id, snapshot });
			print_output!(ctx, &out, |o: &SpaceSnapshotDeleteOutput| {
				println!(
					"{}",
					if o.deleted {
						"Deleted the snapshot"
					} else {
						"No such snapshot"
					}
				);
			});
		}
		SpaceCmd::Export { id, path, current } => {
			let input = SpaceExportInput {
				id,
				path: std::path::absolute(&path)?,
				current_only: current,
			};
			let out: SpaceExportOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceExportOutput| {
				println!(
					"Exported {} files, {} changes, {} versions and {} snapshots ({} bytes) to {}",
					o.summary.files,
					o.summary.changes,
					o.summary.versions,
					o.summary.snapshots,
					o.summary.bytes,
					o.path.display()
				);
			});
		}
		SpaceCmd::Import {
			path,
			destination,
			name,
		} => {
			let input = SpaceImportInput {
				path: std::path::absolute(&path)?,
				destination: std::path::absolute(&destination)?,
				name,
			};
			let out: SpaceImportOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &SpaceImportOutput| {
				println!(
					"Imported {} ({}) with {} files, {} changes, {} versions and {} snapshots",
					o.space.name,
					o.space.id,
					o.summary.files,
					o.summary.changes,
					o.summary.versions,
					o.summary.snapshots
				);
			});
		}
		SpaceCmd::PurgeHistory { id } => {
			let out: SpaceHistoryPurgeOutput = execute_action!(ctx, SpaceHistoryPurgeInput { id });
			print_output!(ctx, &out, |o: &SpaceHistoryPurgeOutput| {
				println!(
					"{}",
					if o.purged {
						"Deleted the space's versions and snapshots"
					} else {
						"The space had no versions"
					}
				);
			});
		}
	}
	Ok(())
}

fn print_snapshots(snapshots: &[SpaceSnapshot]) {
	if snapshots.is_empty() {
		println!("No snapshots");
		return;
	}
	let mut table = Table::new();
	table.load_preset(UTF8_BORDERS_ONLY);
	table.set_header(vec!["ID", "Name", "When", "Files", "Bytes", "By"]);
	for snapshot in snapshots {
		table.add_row(vec![
			snapshot.id.to_string(),
			snapshot.name.clone(),
			snapshot.at.clone(),
			snapshot.files.to_string(),
			snapshot.bytes.to_string(),
			snapshot.created_by.label.clone(),
		]);
	}
	println!("{table}");
}

fn kind(space: &Space) -> &'static str {
	match space.kind {
		SpaceKind::Folder => "folder",
		SpaceKind::Workspace => "workspace",
	}
}

fn location(space: &Space) -> String {
	space
		.path
		.as_ref()
		.map(|path| path.display().to_string())
		.unwrap_or_else(|| format!("{}:/{} (not on this device)", space.source_id, space.root))
}

fn print_space(space: &Space) {
	println!("{} ({})", space.name, space.id);
	println!("  kind:             {}", kind(space));
	println!("  path:             {}", location(space));
	println!(
		"  content indexing: {}",
		if space.content_indexing { "on" } else { "off" }
	);
	println!(
		"  versioning:       {}",
		if space.versioning { "on" } else { "off" }
	);
}
