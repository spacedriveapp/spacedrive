mod setup;

use anyhow::Result;

use crate::context::Context;
use crate::util::prelude::*;

pub async fn run(ctx: &Context) -> Result<()> {
	// Launch interactive setup
	setup::run_interactive(ctx).await?;
	Ok(())
}
