use clap::{Args, Subcommand};
use uuid::Uuid;

use sd_core::ops::libraries::{
	create::input::LibraryCreateInput, delete::input::LibraryDeleteInput,
	info::query::LibraryInfoQueryInput,
};
use sd_core::ops::network::sync_setup::{
	discovery::query::DiscoverRemoteLibrariesInput, input::LibrarySyncAction,
	input::LibrarySyncSetupInput,
};

#[derive(Args, Debug)]
pub struct LibraryCreateArgs {
	pub name: String,
}

impl From<LibraryCreateArgs> for LibraryCreateInput {
	fn from(args: LibraryCreateArgs) -> Self {
		Self::new(args.name)
	}
}

#[derive(Args, Debug)]
pub struct LibraryDeleteArgs {
	pub library_id: Uuid,
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
	#[arg(long, default_value_t = false)]
	pub delete_data: bool,
}

impl From<LibraryDeleteArgs> for LibraryDeleteInput {
	fn from(args: LibraryDeleteArgs) -> Self {
		Self {
			library_id: args.library_id,
			delete_data: args.delete_data,
		}
	}
}

#[derive(Args, Debug)]
pub struct LibraryInfoArgs {
	/// Library ID to get information about (optional, defaults to current library)
	pub library_id: Option<Uuid>,
}

impl LibraryInfoArgs {
	/// Create an input for the specified library ID or current library
	pub fn to_input(
		&self,
		current_library_id: Option<Uuid>,
	) -> anyhow::Result<LibraryInfoQueryInput> {
		let _library_id = self
			.library_id
			.or(current_library_id)
			.ok_or_else(|| anyhow::anyhow!("No library specified and no current library set"))?;

		Ok(LibraryInfoQueryInput {})
	}
}

#[derive(Args, Debug)]
pub struct LibrarySwitchArgs {
	/// Library ID to switch to
	#[arg(long)]
	pub library_id: Option<Uuid>,
	/// Library name to switch to
	#[arg(long)]
	pub name: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum SyncSetupCmd {
	/// Discover libraries on a paired device
	Discover(DiscoverArgs),
	/// Setup library sync between devices
	Setup(SetupArgs),
}

#[derive(Args, Debug)]
pub struct DiscoverArgs {
	/// Device ID to discover libraries from
	pub device_id: Uuid,
}

impl From<DiscoverArgs> for DiscoverRemoteLibrariesInput {
	fn from(args: DiscoverArgs) -> Self {
		Self {
			device_id: args.device_id,
		}
	}
}

#[derive(Args, Debug)]
pub struct SetupArgs {
	/// Local library ID
	#[arg(long)]
	pub local_library: Option<Uuid>,

	/// Remote device ID (paired device)
	#[arg(long)]
	pub remote_device: Option<Uuid>,

	/// Remote library ID (optional for register-only mode)
	#[arg(long)]
	pub remote_library: Option<Uuid>,

	/// Sync action: register-only, create-shared
	#[arg(long)]
	pub action: Option<String>,

	/// Library name for create-shared action
	#[arg(long)]
	pub name: Option<String>,

	/// DEPRICATED: Leader device: "local" or "remote"
	#[arg(long)]
	pub leader: Option<String>,

	/// Local device ID (optional, uses current device if not specified)
	#[arg(long)]
	pub local_device: Option<Uuid>,

	/// Use interactive mode (default if no arguments provided)
	#[arg(short, long)]
	pub interactive: bool,
}

impl SetupArgs {
	pub fn is_interactive(&self) -> bool {
		self.interactive || (self.local_library.is_none() && self.remote_device.is_none())
	}

	pub fn to_input(&self, ctx: &crate::context::Context) -> anyhow::Result<LibrarySyncSetupInput> {
		let local_library = self.local_library
			.ok_or_else(|| anyhow::anyhow!("--local-library is required"))?;
		let remote_device = self.remote_device
			.ok_or_else(|| anyhow::anyhow!("--remote-device is required"))?;

		// Get local device ID from config or argument
		let local_device_id = if let Some(id) = self.local_device {
			id
		} else {
			// Read device config to get current device ID
			let config_path = ctx.data_dir.join("device.json");
			if !config_path.exists() {
				anyhow::bail!("Device config not found. Please specify --local-device");
			}
			let config_data = std::fs::read_to_string(&config_path)?;
			let device_config: sd_core::device::DeviceConfig = serde_json::from_str(&config_data)?;
			device_config.id
		};

		// Determine leader device ID
		let leader = self.leader.as_deref().unwrap_or("local");
		let leader_device_id = match leader {
			"local" => local_device_id,
			"remote" => remote_device,
			_ => anyhow::bail!("Leader must be 'local' or 'remote'"),
		};

		// Parse action
		let action_str = self.action.as_deref().unwrap_or("register-only");
		let action = match action_str {
			"register-only" => LibrarySyncAction::RegisterOnly,
			"create-shared" => {
				let name = self
					.name
					.clone()
					.ok_or_else(|| anyhow::anyhow!("--name is required for create-shared action"))?;
				LibrarySyncAction::CreateShared {
					leader_device_id,
					name,
				}
			}
			_ => anyhow::bail!(
				"Invalid action '{}'. Supported: register-only, create-shared",
				action_str
			),
		};

		Ok(LibrarySyncSetupInput {
			local_device_id,
			remote_device_id: remote_device,
			local_library_id: local_library,
			remote_library_id: self.remote_library,
			action,
			leader_device_id,
		})
	}
}
