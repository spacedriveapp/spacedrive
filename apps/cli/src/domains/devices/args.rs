use clap::Args;

use sd_core::ops::devices::list::query::ListLibraryDevicesInput;

#[derive(Args, Debug, Clone)]
pub struct DevicesListArgs {
	/// Include offline devices
	#[arg(long, default_value_t = true)]
	pub include_offline: bool,

	/// Include detailed information (capabilities, network addresses, etc.)
	#[arg(long, default_value_t = false)]
	pub detailed: bool,

	/// Show paired network devices in addition to library devices
	#[arg(long, default_value_t = false)]
	pub show_paired: bool,
}

impl DevicesListArgs {
	pub fn to_input(&self) -> ListLibraryDevicesInput {
		ListLibraryDevicesInput {
			include_offline: self.include_offline,
			include_details: self.detailed,
			show_paired: self.show_paired,
		}
	}
}
