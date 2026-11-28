//! # sd mount
//!
//! `sd mount` mounts a folder lent by a grant from any Spacedrive node. The
//! thin client needs no library, no pairing and no daemon on this machine:
//! the node's URL and a grant token are enough. The mount is read-only. It
//! runs in the foreground and unmounts on Ctrl-C, on SIGTERM, or when the
//! node stops accepting the token.
//!
//! Linux mounts through FUSE. macOS has no FUSE without a kernel extension,
//! so there the client serves the folder over a localhost NFSv3 server and
//! the system's own `mount_nfs` mounts it, unprivileged.
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

#[cfg(any(
	all(target_os = "linux", feature = "fuse"),
	all(target_os = "macos", feature = "nfs")
))]
pub async fn run(args: MountArgs) -> Result<()> {
	use sd_core::service::mounts::client;

	let dir = frontend::prepare(std::path::absolute(&args.dir)?).await?;
	let fs = client::connect(
		&args.url,
		&args.token,
		args.space.as_deref(),
		args.cache_dir,
	)
	.await?;
	let mounted = frontend::mount(fs.clone(), &dir).await?;
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
	frontend::unmount(mounted, &dir).await;
	fs.close().await;
	println!("Unmounted {}", dir.display());
	if grant_ended {
		anyhow::bail!("the grant was revoked or expired");
	}
	Ok(())
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
mod frontend {
	use std::path::{Path, PathBuf};
	use std::sync::Arc;

	use sd_core::service::mounts::{client::RemoteFs, fuse};

	pub type Mounted = fuse::fs::Mounted;

	pub async fn prepare(dir: PathBuf) -> anyhow::Result<PathBuf> {
		Ok(fuse::prepare_mountpoint(dir).await?)
	}

	pub async fn mount(fs: Arc<RemoteFs>, dir: &Path) -> anyhow::Result<Mounted> {
		Ok(fuse::fs::mount(fs, dir).await?)
	}

	pub async fn unmount(mounted: Mounted, dir: &Path) {
		let result = tokio::task::spawn_blocking(move || mounted.umount_and_join()).await;
		if let Ok(Err(err)) = result {
			// Something still has a file open; detach so the mount ends with it.
			eprintln!("Unmount refused ({err}); detaching");
			let _ = std::process::Command::new("fusermount3")
				.args(["-u", "-z"])
				.arg(dir)
				.status();
		}
	}
}

#[cfg(all(target_os = "macos", feature = "nfs"))]
mod frontend {
	use std::path::{Path, PathBuf};
	use std::sync::Arc;

	use sd_core::service::mounts::{client::RemoteFs, nfs};

	pub type Mounted = nfs::server::NfsServer;

	pub async fn prepare(dir: PathBuf) -> anyhow::Result<PathBuf> {
		Ok(nfs::prepare_mountpoint(dir).await?)
	}

	pub async fn mount(fs: Arc<RemoteFs>, dir: &Path) -> anyhow::Result<Mounted> {
		let server = nfs::server::serve(fs).await?;
		nfs::server::mount(&server, dir).await?;
		Ok(server)
	}

	pub async fn unmount(server: Mounted, dir: &Path) {
		// The unmount forces a busy mount off, so a program still holding a
		// file gets errors rather than keeping the server alive.
		if let Err(err) = nfs::server::unmount(dir).await {
			eprintln!("Unmount refused ({err}); the server stops answering");
		}
		server.shutdown();
	}
}

#[cfg(not(any(
	all(target_os = "linux", feature = "fuse"),
	all(target_os = "macos", feature = "nfs")
)))]
pub async fn run(_args: MountArgs) -> Result<()> {
	anyhow::bail!("sd mount needs Linux with the fuse feature or macOS with the nfs feature")
}
