//! # sd nfs
//!
//! `sd nfs`: mount sources as a read-only directory tree through a localhost
//! NFS server, for macOS, where FUSE needs a kernel extension.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::mounts::{NfsListInput, NfsListOutput, NfsMountInput, NfsUnmountInput};
use sd_core::service::mounts::nfs::NfsMountInfo;

#[derive(Subcommand, Debug)]
pub enum NfsCmd {
	/// Mount every source, or one grant's folder, at an empty directory
	Mount(NfsMountArgs),
	/// Unmount a mount the daemon holds
	Unmount(NfsPathArgs),
	/// List the daemon's mounts
	List,
}

#[derive(Args, Debug)]
pub struct NfsMountArgs {
	/// An existing, empty directory
	pub path: PathBuf,
	/// Show only the folder this grant lends. The grant is checked on every
	/// listing and open, and the mount goes away when the grant ends.
	#[arg(long)]
	pub grant: Option<String>,
}

#[derive(Args, Debug)]
pub struct NfsPathArgs {
	pub path: PathBuf,
}

/// The daemon resolves the path in its own working directory, so a relative
/// one is made absolute here.
fn absolute(path: &Path) -> Result<PathBuf> {
	Ok(std::path::absolute(path)?)
}

pub async fn run(ctx: &Context, cmd: NfsCmd) -> Result<()> {
	match cmd {
		NfsCmd::Mount(args) => {
			let input = NfsMountInput {
				path: absolute(&args.path)?,
				grant_token: args.grant,
			};
			let out: NfsMountInfo = execute_core_action!(ctx, input);
			print_output!(ctx, &out, |o: &NfsMountInfo| {
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
		NfsCmd::Unmount(args) => {
			let input = NfsUnmountInput {
				path: absolute(&args.path)?,
			};
			let out: NfsMountInfo = execute_core_action!(ctx, input);
			print_output!(ctx, &out, |o: &NfsMountInfo| {
				println!("Unmounted {}", o.path.display());
			});
		}
		NfsCmd::List => {
			let out: NfsListOutput = execute_core_query!(ctx, NfsListInput::default());
			print_output!(ctx, &out, |o: &NfsListOutput| {
				if !o.supported {
					println!(
						"This daemon has no NFS mount support (macOS or Linux, built with the nfs feature)"
					);
				} else if !o.enabled {
					println!("NFS mounts are off: sd-cli config set mounts.nfs_enabled true");
				}
				if o.mounts.is_empty() {
					println!("No mounts");
					return;
				}
				let mut table = Table::new();
				table.load_preset(UTF8_BORDERS_ONLY);
				table.set_header(vec!["Path", "Port", "Shows", "Grant", "Expires"]);
				for mount in &o.mounts {
					table.add_row(vec![
						mount.path.display().to_string(),
						mount.port.to_string(),
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
