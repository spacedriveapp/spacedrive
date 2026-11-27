//! `sd space`: create, list, show, update and delete spaces.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use uuid::Uuid;

use sd_core::{
	domain::{Space, SpaceKind},
	ops::spaces::{
		SpaceCreateInput, SpaceCreateOutput, SpaceDeleteInput, SpaceDeleteOutput,
		SpaceGetQueryInput, SpaceGetQueryOutput, SpaceUpdateInput, SpaceUpdateOutput,
		SpacesListQueryInput, SpacesListQueryOutput,
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
	/// Rename a space or turn its content indexing on or off
	Update(SpaceUpdateArgs),
	/// Stop treating a folder as a space; its files stay
	Delete { id: Uuid },
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
}

pub async fn run(ctx: &Context, cmd: SpaceCmd) -> Result<()> {
	match cmd {
		SpaceCmd::Create(args) => {
			let path = match args.path {
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
				content_indexing: Some(!args.no_content_indexing),
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
	}
	Ok(())
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
}
