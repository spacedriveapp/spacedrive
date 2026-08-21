//! Query for the mounts share: where it listens and what it serves.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::mounts,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountShare {
	pub name: String,
	pub source_id: Uuid,
	pub root: PathBuf,
	pub attached: bool,
	pub url: String,
	/// Owning device label for replicated peer sources; None for local.
	pub device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsStatus {
	pub running: bool,
	/// WebDAV base, kept as plain interop and as a measurement baseline.
	pub base_url: Option<String>,
	/// The URL a host SMB client mounts — the one to actually use.
	/// Credentials are included: the server is loopback-only and the
	/// password is regenerated every start unless pinned by environment.
	pub smb_url: Option<String>,
	/// Ready-to-paste mount command for this platform.
	pub smb_mount_hint: Option<String>,
	pub shares: Vec<MountShare>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsStatusQuery {
	#[allow(dead_code)]
	input: MountsStatusInput,
}

impl CoreQuery for MountsStatusQuery {
	type Input = MountsStatusInput;
	type Output = MountsStatus;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let addr = mounts::bound_addr();
		let base_url = addr.map(|a| format!("http://{a}/dav"));

		let shares = match &base_url {
			None => Vec::new(),
			Some(base) => {
				let mut shares: Vec<MountShare> = context
					.ephemeral_cache()
					.sources()
					.into_iter()
					.map(|s| {
						let name = mounts::share_name(&s.root, s.id);
						let url = format!("{base}/{name}/");
						MountShare {
							name,
							source_id: s.id,
							root: s.root,
							attached: s.attached,
							url,
							device: None,
						}
					})
					.collect();
				for remote in mounts::peer::remote_shares().await {
					let name = mounts::remote_share_name(&remote);
					let url = format!("{base}/{name}/");
					shares.push(MountShare {
						name,
						source_id: remote.info.id,
						root: remote.info.root.clone(),
						attached: remote.info.attached,
						url,
						device: Some(remote.device_label.clone()),
					});
				}
				shares
			}
		};

		let smb_url = mounts::smb::mount_url();
		let smb_mount_hint =
			smb_url.as_ref().map(|url| {
				let target = url.trim_start_matches("smb://");
				if cfg!(target_os = "macos") {
					format!("mkdir -p /tmp/sdmnt && mount_smbfs -o nobrowse,soft \"//{target}\" /tmp/sdmnt")
				} else if cfg!(target_os = "windows") {
					format!("net use * \\\\{}", target.replace('/', "\\"))
				} else {
					format!("mount -t cifs //{target} /mnt/spacedrive")
				}
			});

		Ok(MountsStatus {
			running: addr.is_some(),
			base_url,
			smb_url,
			smb_mount_hint,
			shares,
		})
	}
}

crate::register_core_query!(MountsStatusQuery, "mounts.status");
