//! `sd fuse`: mount sources as a read-only directory tree on Linux.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::mounts::{FuseListInput, FuseListOutput, FuseMountInput, FuseUnmountInput};
use sd_core::service::mounts::fuse::FuseMountInfo;

#[derive(Subcommand, Debug)]
pub enum FuseCmd {
	/// Mount every source, or one grant's folder, at an empty directory
	Mount(FuseMountArgs),
	/// Unmount a mount the daemon holds
	Unmount(FusePathArgs),
	/// List the daemon's mounts
	List,
}

#[derive(Args, Debug)]
pub struct FuseMountArgs {
	/// An existing, empty directory
	pub path: PathBuf,
	/// Show only the folder this grant lends. The grant is checked on every
	/// listing and open, and the mount goes away when the grant ends.
	#[arg(long)]
	pub grant: Option<String>,
}

#[derive(Args, Debug)]
pub struct FusePathArgs {
	pub path: PathBuf,
}

/// The daemon resolves the path in its own working directory, so a relative
/// one is made absolute here.
fn absolute(path: &Path) -> Result<PathBuf> {
	Ok(std::path::absolute(path)?)
}

pub async fn run(ctx: &Context, cmd: FuseCmd) -> Result<()> {
	match cmd {
		FuseCmd::Mount(args) => {
			let input = FuseMountInput {
				path: absolute(&args.path)?,
				grant_token: args.grant,
			};
			let out: FuseMountInfo = execute_core_action!(ctx, input);
			print_output!(ctx, &out, |o: &FuseMountInfo| {
				match &o.granted_path {
					Some(granted) => println!(
						"Mounted {} at {} until {}",
						granted.display(),
						o.path.display(),
						o.expires_at.map(|t| t.to_rfc3339()).unwrap_or_default()
					),
					None => println!("Mounted every source at {}", o.path.display()),
				}
			});
		}
		FuseCmd::Unmount(args) => {
			let input = FuseUnmountInput {
				path: absolute(&args.path)?,
			};
			let out: FuseMountInfo = execute_core_action!(ctx, input);
			print_output!(ctx, &out, |o: &FuseMountInfo| {
				println!("Unmounted {}", o.path.display());
			});
		}
		FuseCmd::List => {
			let out: FuseListOutput = execute_core_query!(ctx, FuseListInput::default());
			print_output!(ctx, &out, |o: &FuseListOutput| {
				if !o.supported {
					println!(
						"This daemon has no FUSE support (Linux, built with the fuse feature)"
					);
				} else if !o.enabled {
					println!("FUSE mounts are off: sd-cli config set mounts.fuse_enabled true");
				}
				if o.mounts.is_empty() {
					println!("No mounts");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["Path", "Shows", "Grant", "Expires"]);
				for mount in &o.mounts {
					table.add_row(vec![
						mount.path.display().to_string(),
						mount
							.granted_path
							.as_ref()
							.map(|p| p.display().to_string())
							.unwrap_or_else(|| "every source".to_string()),
						mount.grant_id.map(|id| id.to_string()).unwrap_or_default(),
						mount.expires_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
					]);
				}
				println!("{table}");
			});
		}
	}
	Ok(())
}
