//! Redundancy CLI domain
//!
//! Commands for inspecting data redundancy across volumes:
//! - `summary` — per-volume at-risk vs redundant breakdown + replication score
//! - `at-risk` — list files whose content lives on only one volume
//! - `compare` — diff two volumes (unique-to-A, unique-to-B, shared)

mod args;

use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Attribute, Cell, Table};

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::redundancy::summary::{RedundancySummaryInput, RedundancySummaryOutput};
use sd_core::ops::search::{
	input::{
		FileSearchInput, PaginationOptions, SearchFilters, SearchMode, SearchScope, SortDirection,
		SortField, SortOptions,
	},
	output::FileSearchOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum RedundancyCmd {
	/// Show redundancy summary (replication score + per-volume breakdown)
	Summary(SummaryArgs),
	/// List at-risk files (content that only lives on one volume)
	AtRisk(AtRiskArgs),
	/// Compare two volumes (unique-to-A, unique-to-B, or shared)
	Compare(CompareArgs),
}

pub async fn run(ctx: &Context, cmd: RedundancyCmd) -> Result<()> {
	match cmd {
		RedundancyCmd::Summary(args) => run_summary(ctx, args).await,
		RedundancyCmd::AtRisk(args) => run_at_risk(ctx, args).await,
		RedundancyCmd::Compare(args) => run_compare(ctx, args).await,
	}
}

async fn run_summary(ctx: &Context, args: SummaryArgs) -> Result<()> {
	let input: RedundancySummaryInput = args.into();
	let out: RedundancySummaryOutput = execute_query!(ctx, input);

	print_output!(ctx, &out, |o: &RedundancySummaryOutput| {
		render_summary(o);
	});
	Ok(())
}

async fn run_at_risk(ctx: &Context, args: AtRiskArgs) -> Result<()> {
	let input = FileSearchInput {
		query: String::new(),
		scope: SearchScope::Library,
		mode: SearchMode::Fast,
		filters: SearchFilters {
			at_risk: Some(!args.redundant),
			on_volumes: args.volume.map(|v| vec![v]),
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::Size,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: args.limit,
			offset: args.offset,
		},
	};

	let out: FileSearchOutput = execute_query!(ctx, input);
	let label = if args.redundant {
		"redundant"
	} else {
		"at-risk"
	};

	print_output!(ctx, &out, |o: &FileSearchOutput| {
		render_file_list(o, label);
	});
	Ok(())
}

async fn run_compare(ctx: &Context, args: CompareArgs) -> Result<()> {
	let (on, not_on, min_count, label) = match args.mode {
		CompareMode::UniqueA => (
			Some(vec![args.volume_a]),
			Some(vec![args.volume_b]),
			None,
			"unique to A",
		),
		CompareMode::UniqueB => (
			Some(vec![args.volume_b]),
			Some(vec![args.volume_a]),
			None,
			"unique to B",
		),
		CompareMode::Shared => (
			Some(vec![args.volume_a, args.volume_b]),
			None,
			Some(2u32),
			"shared",
		),
	};

	let input = FileSearchInput {
		query: String::new(),
		scope: SearchScope::Library,
		mode: SearchMode::Fast,
		filters: SearchFilters {
			on_volumes: on,
			not_on_volumes: not_on,
			min_volume_count: min_count,
			..Default::default()
		},
		sort: SortOptions {
			field: SortField::Size,
			direction: SortDirection::Desc,
		},
		pagination: PaginationOptions {
			limit: args.limit,
			offset: args.offset,
		},
	};

	let out: FileSearchOutput = execute_query!(ctx, input);

	print_output!(ctx, &out, |o: &FileSearchOutput| {
		render_file_list(o, label);
	});
	Ok(())
}

// ─── rendering helpers ────────────────────────────────────────────────────────

fn render_summary(o: &RedundancySummaryOutput) {
	let totals = &o.library_totals;

	// Library-wide header
	let mut overview = Table::new();
	overview.load_preset(UTF8_BORDERS_ONLY);
	overview.set_header(vec![
		Cell::new("Redundancy Summary").add_attribute(Attribute::Bold),
		Cell::new(""),
	]);
	overview.add_row(vec![
		Cell::new("Unique content"),
		Cell::new(format_bytes_i64(totals.total_unique_content_bytes)),
	]);
	overview.add_row(vec![
		Cell::new("At risk"),
		Cell::new(format!(
			"{} ({})",
			format_bytes_i64(totals.total_at_risk_bytes),
			percent_of(
				totals.total_at_risk_bytes,
				totals.total_unique_content_bytes
			),
		)),
	]);
	overview.add_row(vec![
		Cell::new("Redundant"),
		Cell::new(format!(
			"{} ({})",
			format_bytes_i64(totals.total_redundant_bytes),
			percent_of(
				totals.total_redundant_bytes,
				totals.total_unique_content_bytes
			),
		)),
	]);
	overview.add_row(vec![
		Cell::new("Replication score"),
		Cell::new(format!(
			"{:.2} ({:.1}%)",
			totals.replication_score,
			totals.replication_score * 100.0
		)),
	]);
	println!("{}", overview);
	println!();

	// Per-volume breakdown
	if o.volumes.is_empty() {
		println!("No volumes with indexed content found.");
		return;
	}

	let mut table = Table::new();
	table.load_preset(UTF8_BORDERS_ONLY);
	table.set_header(vec![
		Cell::new("Volume").add_attribute(Attribute::Bold),
		Cell::new("Total").add_attribute(Attribute::Bold),
		Cell::new("At-Risk").add_attribute(Attribute::Bold),
		Cell::new("Redundant").add_attribute(Attribute::Bold),
		Cell::new("Files").add_attribute(Attribute::Bold),
	]);

	for v in &o.volumes {
		let name = v
			.display_name
			.as_deref()
			.map(|n| n.to_string())
			.unwrap_or_else(|| v.volume_uuid.to_string());

		let at_risk = format!(
			"{} ({})",
			format_bytes_i64(v.at_risk_bytes),
			percent_of(v.at_risk_bytes, v.total_bytes),
		);
		let redundant = format!(
			"{} ({})",
			format_bytes_i64(v.redundant_bytes),
			percent_of(v.redundant_bytes, v.total_bytes),
		);
		let files = format!(
			"{} ({} at-risk / {} redundant)",
			v.total_file_count, v.at_risk_file_count, v.redundant_file_count
		);

		table.add_row(vec![
			Cell::new(name),
			Cell::new(format_bytes_i64(v.total_bytes)),
			Cell::new(at_risk),
			Cell::new(redundant),
			Cell::new(files),
		]);

		// UUID on a subline for precision
		table.add_row(vec![
			Cell::new(format!("  {}", v.volume_uuid)),
			Cell::new(""),
			Cell::new(""),
			Cell::new(""),
			Cell::new(""),
		]);
	}

	println!("{}", table);
}

fn render_file_list(o: &FileSearchOutput, label: &str) {
	if o.files.is_empty() {
		println!("No {} files found.", label);
		return;
	}

	println!(
		"Showing {} of {} {} file(s) ({}ms)",
		o.files.len(),
		o.total_found,
		label,
		o.execution_time_ms
	);
	println!();

	let mut table = Table::new();
	table.load_preset(UTF8_BORDERS_ONLY);
	table.set_header(vec![
		Cell::new("#").add_attribute(Attribute::Bold),
		Cell::new("Name").add_attribute(Attribute::Bold),
		Cell::new("Size").add_attribute(Attribute::Bold),
		Cell::new("Ext").add_attribute(Attribute::Bold),
		Cell::new("Modified").add_attribute(Attribute::Bold),
		Cell::new("Path").add_attribute(Attribute::Bold),
	]);

	for (i, f) in o.files.iter().enumerate() {
		table.add_row(vec![
			Cell::new((i + 1).to_string()),
			Cell::new(truncate(&f.name, 48)),
			Cell::new(format_bytes_u64(f.size)),
			Cell::new(f.extension.clone().unwrap_or_default()),
			Cell::new(f.modified_at.format("%Y-%m-%d %H:%M").to_string()),
			Cell::new(truncate(&f.sd_path.display().to_string(), 60)),
		]);
	}

	println!("{}", table);
}

fn percent_of(part: i64, total: i64) -> String {
	if total <= 0 {
		return "0%".into();
	}
	format!("{:.1}%", (part as f64 / total as f64) * 100.0)
}

fn format_bytes_i64(bytes: i64) -> String {
	if bytes < 0 {
		return format!("-{}", format_bytes_u64(bytes.unsigned_abs()));
	}
	format_bytes_u64(bytes as u64)
}

fn format_bytes_u64(bytes: u64) -> String {
	const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
	let mut size = bytes as f64;
	let mut unit = 0;
	while size >= 1024.0 && unit < UNITS.len() - 1 {
		size /= 1024.0;
		unit += 1;
	}
	if unit == 0 {
		format!("{} {}", bytes, UNITS[unit])
	} else {
		format!("{:.1} {}", size, UNITS[unit])
	}
}

fn truncate(s: &str, max: usize) -> String {
	if s.chars().count() <= max {
		s.to_string()
	} else {
		let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
		out.push('…');
		out
	}
}
