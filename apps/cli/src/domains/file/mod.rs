mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::util::prelude::*;

use crate::context::Context;
use sd_core::ops::files::copy::job::FileCopyOutput;

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum FileCmd {
	/// Copy files
	Copy(FileCopyArgs),
}

pub async fn run(ctx: &Context, cmd: FileCmd) -> Result<()> {
	match cmd {
		FileCmd::Copy(args) => {
			let input: sd_core::ops::files::copy::input::FileCopyInput = args.into();
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "))
			}
			let out: FileCopyOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &FileCopyOutput| {
				println!(
					"Copy request submitted - {} files copied ({} bytes)",
					o.copied_count, o.total_bytes
				);
			});
		}
	}
	Ok(())
}
