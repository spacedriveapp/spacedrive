use crate::{context::Context, util::prelude::*};
use anyhow::Result;
use clap::Subcommand;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, Table};
use sd_core::ops::layouts::{LayoutsListQuery, LayoutsListQueryInput};

#[derive(Debug, Clone, Subcommand)]
pub enum LayoutsCmd {
	/// List all layouts
	List,
	/// Show a layout's groups and items
	Show {
		/// Layout ID
		layout_id: String,
	},
}

pub async fn exec(cmd: LayoutsCmd, ctx: &Context) -> Result<()> {
	match cmd {
		LayoutsCmd::List => list_layouts(ctx).await,
		LayoutsCmd::Show { layout_id } => get_sidebar(ctx, layout_id).await,
	}
}

async fn list_layouts(ctx: &Context) -> Result<()> {
	// Get current library ID
	let library_id = ctx.library_id.ok_or_else(|| {
		anyhow::anyhow!("No library selected. Run 'sd library list' to see available libraries")
	})?;

	println!("Library ID: {}", library_id);

	// Create query input
	let input = LayoutsListQueryInput;

	// Execute query through the client
	let response = ctx.core.query(&input, Some(library_id)).await?;

	println!(
		"\nRaw response: {}",
		serde_json::to_string_pretty(&response)?
	);

	let result: sd_core::ops::layouts::LayoutsListOutput = serde_json::from_value(response)
		.map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

	println!("\nQuery executed successfully!");
	println!("Found {} layouts:", result.layouts.len());

	if result.layouts.is_empty() {
		println!("  (no layouts found)");
	} else {
		let mut table = Table::new();
		table.load_preset(UTF8_BORDERS_ONLY);
		table.set_header(vec!["ID", "Name", "Icon", "Color", "Order"]);

		for layout in result.layouts {
			table.add_row(vec![
				Cell::new(&layout.id.to_string()[..8]),
				Cell::new(&layout.name),
				Cell::new(&layout.icon),
				Cell::new(&layout.color),
				Cell::new(layout.order),
			]);
		}

		println!("{table}");
	}

	Ok(())
}

async fn get_sidebar(ctx: &Context, layout_id: String) -> Result<()> {
	use sd_core::ops::layouts::{SidebarLayoutQuery, SidebarLayoutQueryInput};
	use uuid::Uuid;

	let library_id = ctx
		.library_id
		.ok_or_else(|| anyhow::anyhow!("No library selected"))?;

	let layout_uuid = Uuid::parse_str(&layout_id)?;

	let input = SidebarLayoutQueryInput {
		layout_id: layout_uuid,
	};
	let response: serde_json::Value = ctx.core.query(&input, Some(library_id)).await?;

	println!("\nRaw layout response:");
	println!("{}", serde_json::to_string_pretty(&response)?);

	Ok(())
}
