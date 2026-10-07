use anyhow::Result;
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::PathBuf;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::library::AddOverrides;
use sd_core::ops::indexing::sources::StorePlacement;
use sd_core::ops::mounts::{MountsReplicationSetPausedInput, MountsReplicationSetPausedOutput};
use sd_core::ops::sources::{
	delete::action::{DeleteSourceInput, DeleteSourceOutput},
	freeze::action::{FreezeSourceInput, FreezeSourceOutput},
	list::{output::SourceInfo, query::ListSourcesInput},
	track::action::{TrackSourceInput, TrackSourceOutput},
	update::action::{UpdateSourceInput, UpdateSourceOutput},
	verify::action::{VerifySourceInput, VerifySourceOutput},
};

#[derive(Subcommand, Debug)]
pub enum SourceCmd {
	/// Track a root as a source and start indexing it
	Track(SourceTrackArgs),
	/// List registered sources
	List,
	/// Write a dated, self-contained copy of a source's store
	Freeze(SourceFreezeArgs),
	/// Read every byte of the source's duplicate files, upgrading their
	/// content identity from candidate to confirmed
	Verify(SourceVerifyArgs),
	/// Rename a source or change its capture policy
	Update(SourceUpdateArgs),
	/// Remove a source from the library, keeping its catalog unless asked
	Untrack(SourceUntrackArgs),
	/// Control how this device copies paired devices' source indexes
	#[command(subcommand)]
	Replication(ReplicationCmd),
}

#[derive(Subcommand, Debug)]
pub enum ReplicationCmd {
	/// Stop fetching replicas: no new transfer starts and any in flight
	/// stops, keeping its partial file. Persists across daemon restarts.
	Pause,
	/// Fetch replicas again, continuing partial transfers where they stopped
	Resume,
}

#[derive(Args, Debug)]
pub struct SourceTrackArgs {
	/// The root to track. A mount point tracks the whole drive; any path
	/// under one tracks that subtree.
	pub path: PathBuf,
	/// Display name, or the directory's own name
	#[arg(long)]
	pub name: Option<String>,
	/// Record everything readable, skipping the rules that hide system
	/// files, .git and dev directories. Archival drives want this.
	#[arg(long)]
	pub unfiltered: bool,
	/// Keep the catalog on the source itself, under .spacedrive, instead of
	/// in the library's data directory
	#[arg(long, conflicts_with = "in_library")]
	pub on_source: bool,
	/// Keep the catalog in the library's data directory
	#[arg(long)]
	pub in_library: bool,
	/// For an on-source catalog, whether the library keeps an offline copy
	#[arg(long)]
	pub keep_offline_copy: Option<bool>,
	/// Skip content identification after the walk
	#[arg(long)]
	pub no_identify: bool,
}

#[derive(Args, Debug)]
pub struct SourceFreezeArgs {
	/// The source's id, from `sources list`
	pub source_id: String,
}

#[derive(Args, Debug)]
pub struct SourceVerifyArgs {
	/// The source's id, from `sources list`
	pub source_id: String,
}

#[derive(Args, Debug)]
pub struct SourceUntrackArgs {
	/// The source's id, from `sources list`
	pub source_id: String,
	/// Also delete the catalog: records, content evidence and assertions
	#[arg(long)]
	pub delete_catalog: bool,
}

#[derive(Args, Debug)]
pub struct SourceUpdateArgs {
	/// The source's id, from `sources list`
	pub source_id: String,
	/// New display name
	#[arg(long)]
	pub name: Option<String>,
	/// Record everything readable (true), or apply the default rules to new
	/// captures (false). Widening dispatches a walk for what was skipped;
	/// narrowing removes nothing, since hiding is view-time.
	#[arg(long)]
	pub unfiltered: Option<bool>,
}

pub async fn run(ctx: &Context, cmd: SourceCmd) -> Result<()> {
	match cmd {
		SourceCmd::Track(args) => {
			let path = args
				.path
				.canonicalize()
				.map_err(|e| anyhow::anyhow!("{}: {e}", args.path.display()))?;
			let input = TrackSourceInput {
				path,
				name: args.name,
				overrides: AddOverrides {
					placement: if args.on_source {
						Some(StorePlacement::OnSource)
					} else if args.in_library {
						Some(StorePlacement::InLibrary)
					} else {
						None
					},
					keep_offline_copy: args.keep_offline_copy,
					unfiltered: args.unfiltered.then_some(true),
					identify_content: args.no_identify.then_some(false),
				},
			};

			let out: TrackSourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &TrackSourceOutput| {
				println!("Tracking {} as source {}", o.root.display(), o.id);
				if let Some(store) = &o.store_path {
					println!(
						"Catalog {} at {}",
						if o.catalog_reused {
							"reopened"
						} else {
							"started"
						},
						store.display()
					);
				}
				if let Some(job) = o.job_id {
					println!("Indexing started (job {job})");
				}
			});
		}
		SourceCmd::List => {
			let out: Vec<SourceInfo> = execute_query!(ctx, ListSourcesInput { data_type: None });
			print_output!(ctx, &out, |sources: &Vec<SourceInfo>| {
				if sources.is_empty() {
					println!("No sources registered");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["ID", "Name", "Type", "Records", "Status", "Root"]);
				for source in sources {
					let mut status = if source.attached {
						source.status.clone()
					} else {
						format!("{} (detached)", source.status)
					};
					if let Some(transfer) = &source.transfer {
						status = format!(
							"{status}: fetching {}",
							crate::util::output::format_transfer(
								transfer.bytes,
								transfer.total,
								transfer.bytes_per_sec
							)
						);
					}
					table.add_row(vec![
						source.id.to_string(),
						source.name.clone(),
						source.data_type.clone(),
						source.item_count.to_string(),
						status,
						source.root.clone().unwrap_or_default(),
					]);
				}
				println!("{table}");
			});
		}
		SourceCmd::Update(args) => {
			let input = UpdateSourceInput {
				source_id: args.source_id,
				name: args.name,
				unfiltered: args.unfiltered,
			};

			let out: UpdateSourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &UpdateSourceOutput| {
				println!(
					"{} — capture: {}",
					o.name,
					if o.unfiltered {
						"everything"
					} else {
						"default rules"
					}
				);
				if let Some(job) = o.rewalk_job {
					println!("Walking for what the rules skipped (job {job})");
				}
			});
		}
		SourceCmd::Verify(args) => {
			let input = VerifySourceInput {
				source_id: args.source_id,
			};

			let out: VerifySourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &VerifySourceOutput| {
				println!(
					"Verifying {} shared-content files (job {})",
					o.outstanding, o.job_id
				);
			});
		}
		SourceCmd::Replication(cmd) => {
			let input = MountsReplicationSetPausedInput {
				paused: matches!(cmd, ReplicationCmd::Pause),
			};
			let out: MountsReplicationSetPausedOutput = execute_core_action!(ctx, input);
			print_output!(ctx, &out, |o: &MountsReplicationSetPausedOutput| {
				println!("{}", o.message);
			});
		}
		SourceCmd::Untrack(args) => {
			let input = DeleteSourceInput {
				source_id: args.source_id,
				delete_catalog: args.delete_catalog,
			};

			let out: DeleteSourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &DeleteSourceOutput| {
				if o.catalog_deleted {
					println!("Removed the source and deleted its catalog");
				} else if let Some(path) = &o.catalog_path {
					println!(
						"Removed the source; its catalog stays at {} and is reopened by tracking the same path again",
						path.display()
					);
				} else {
					println!("Removed the source");
				}
			});
		}
		SourceCmd::Freeze(args) => {
			let input = FreezeSourceInput {
				source_id: args.source_id,
			};

			let out: FreezeSourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &FreezeSourceOutput| {
				println!("Frozen {} records into {}", o.records, o.path);
			});
		}
	}

	Ok(())
}
