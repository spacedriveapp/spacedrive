//! # sd mount
//!
//! `sd mount` mounts a folder lent by a grant from any Spacedrive node. The
//! thin client needs no library, no pairing and no daemon on this machine:
//! the node's URL and a grant token are enough. The mount is read-only. It
//! runs in the foreground and unmounts on Ctrl-C, on SIGTERM, or when the
//! node stops accepting the token.
//!
//! ## Example
//! ```bash
//! SD_GRANT_TOKEN=sdg_... sd mount http://127.0.0.1:7765 ~/site
//! ```

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct MountArgs {
	/// The node's agent endpoint, for example http://127.0.0.1:7765
	pub url: String,
	/// An existing, empty directory to mount at
	pub dir: PathBuf,
	/// The grant token (sdg_...). Prefer SD_GRANT_TOKEN, which other users
	/// cannot read from the process list.
	#[arg(long, env = "SD_GRANT_TOKEN", hide_env_values = true)]
	pub token: String,
	/// The space to mount, when the token reaches more than one
	#[arg(long)]
	pub space: Option<String>,
	/// Where to keep fetched blocks; defaults to the user's cache directory
	#[arg(long)]
	pub cache_dir: Option<PathBuf>,
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
pub async fn run(args: MountArgs) -> Result<()> {
	use sd_core::service::mounts::{client, fuse};
	use std::sync::Arc;

	let dir = fuse::prepare_mountpoint(std::path::absolute(&args.dir)?).await?;
	let fs = client::connect(
		&args.url,
		&args.token,
		args.space.as_deref(),
		args.cache_dir,
	)
	.await?;
	let mounted = fuse::fs::mount(fs.clone() as Arc<_>, &dir).await?;
	println!(
		"Mounted {} at {} (read-only). Press Ctrl-C to unmount.",
		args.url,
		dir.display()
	);

	let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
	let grant_ended = tokio::select! {
		_ = tokio::signal::ctrl_c() => false,
		_ = terminate.recv() => false,
		_ = fs.grant_ended() => true,
	};
	if grant_ended {
		eprintln!("The node no longer accepts the grant token; unmounting");
	}
	let result = tokio::task::spawn_blocking(move || mounted.umount_and_join()).await?;
	fs.close().await;
	if let Err(err) = result {
		// Something still has a file open; detach so the mount ends with it.
		eprintln!("Unmount refused ({err}); detaching");
		let _ = std::process::Command::new("fusermount3")
			.args(["-u", "-z"])
			.arg(&dir)
			.status();
	}
	println!("Unmounted {}", dir.display());
	if grant_ended {
		anyhow::bail!("the grant was revoked or expired");
	}
	Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "fuse")))]
pub async fn run(_args: MountArgs) -> Result<()> {
	anyhow::bail!("sd mount needs Linux and a build with the fuse feature")
}
