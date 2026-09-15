use anyhow::Result;
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::PathBuf;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::sources::{
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
				unfiltered: args.unfiltered,
			};

			let out: TrackSourceOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &TrackSourceOutput| {
				println!("Tracking {} as source {}", o.root.display(), o.id);
				if let Some(job) = o.job_id {
					println!("Indexing started (job {job})");
				}
			});
		}
		SourceCmd::List => {
			let out: Vec<SourceInfo> =
				execute_query!(ctx, ListSourcesInput { data_type: None });
			print_output!(ctx, &out, |sources: &Vec<SourceInfo>| {
				if sources.is_empty() {
					println!("No sources registered");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["ID", "Name", "Type", "Records", "Status", "Root"]);
				for source in sources {
					table.add_row(vec![
						source.id.to_string(),
						source.name.clone(),
						source.data_type.clone(),
						source.item_count.to_string(),
						if source.attached {
							source.status.clone()
						} else {
							format!("{} (detached)", source.status)
						},
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
					if o.unfiltered { "everything" } else { "default rules" }
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
