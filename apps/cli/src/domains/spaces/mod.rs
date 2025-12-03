use crate::{context::Context, util::prelude::*};
use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, Table};
use sd_core::ops::spaces::{SpacesListQuery, SpacesListQueryInput};

#[derive(Debug, Clone, Subcommand)]
pub enum SpacesCmd {
	/// List all spaces
	List,
	/// Get space layout
	Layout {
		/// Space ID
		space_id: String,
	},
}

pub async fn exec(cmd: SpacesCmd, ctx: &Context) -> Result<()> {
	match cmd {
		SpacesCmd::List => list_spaces(ctx).await,
		SpacesCmd::Layout { space_id } => get_layout(ctx, space_id).await,
	}
}

async fn list_spaces(ctx: &Context) -> Result<()> {
	// Get current library ID
	let library_id = ctx.library_id.ok_or_else(|| {
		anyhow::anyhow!("No library selected. Run 'sd library list' to see available libraries")
	})?;

	println!("Library ID: {}", library_id);

	// Create query input
	let input = SpacesListQueryInput;

	// Execute query through the client
	let response = ctx.core.query(&input, Some(library_id)).await?;

	println!(
		"\nRaw response: {}",
		serde_json::to_string_pretty(&response)?
	);

	let result: sd_core::ops::spaces::SpacesListOutput = serde_json::from_value(response)
		.map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

	println!("\nQuery executed successfully!");
	println!("Found {} spaces:", result.spaces.len());

	if result.spaces.is_empty() {
		println!("  (no spaces found)");
	} else {
		let mut table = Table::new();
		table.load_preset(UTF8_BORDERS_ONLY);
		table.set_header(vec!["ID", "Name", "Icon", "Color", "Order"]);

		for space in result.spaces {
			table.add_row(vec![
				Cell::new(&space.id.to_string()[..8]),
				Cell::new(&space.name),
				Cell::new(&space.icon),
				Cell::new(&space.color),
				Cell::new(space.order),
			]);
		}

		println!("{table}");
	}

	Ok(())
}

async fn get_layout(ctx: &Context, space_id: String) -> Result<()> {
	use sd_core::ops::spaces::{SpaceLayoutQuery, SpaceLayoutQueryInput};
	use uuid::Uuid;

	let library_id = ctx
		.library_id
		.ok_or_else(|| anyhow::anyhow!("No library selected"))?;

	let space_uuid = Uuid::parse_str(&space_id)?;

	let input = SpaceLayoutQueryInput {
		space_id: space_uuid,
	};
	let response: serde_json::Value = ctx.core.query(&input, Some(library_id)).await?;

	println!("\nRaw layout response:");
	println!("{}", serde_json::to_string_pretty(&response)?);

	Ok(())
}
