use std::io::Error;
use std::process::Command;
use std::str;

use serde::{Deserialize, Serialize};
use specta::Type;
use strum_macros::{Display, EnumIter};

#[repr(i32)]
#[derive(Debug, Clone, Display, Copy, EnumIter, Type, Serialize, Deserialize, Eq, PartialEq)]
pub enum HardwareModel {
	Other,
	MacStudio,
	MacBookAir,
	MacBookPro,
	MacBook,
	MacMini,
	MacPro,
	IMac,
	IMacPro,
	IPad,
	IPhone,
}

impl HardwareModel {
	pub fn from_display_name(name: &str) -> Self {
		use strum::IntoEnumIterator;
		HardwareModel::iter()
			.find(|&model| {
				model.to_string().to_lowercase().replace(' ', "")
					== name.to_lowercase().replace(' ', "")
			})
			.unwrap_or(HardwareModel::Other)
	}
}

pub fn get_hardware_model_name() -> Result<HardwareModel, Error> {
	#[cfg(target_os = "macos")]
	{
		let output = Command::new("system_profiler")
			.arg("SPHardwareDataType")
			.output()?;

		if output.status.success() {
			let output_str = std::str::from_utf8(&output.stdout).unwrap_or_default();
			let hardware_model = output_str
				.lines()
				.find(|line| line.to_lowercase().contains("model name"))
				.and_then(|line| line.split_once(':'))
				.map(|(_, model_name)| HardwareModel::from_display_name(model_name.trim()))
				.unwrap_or(HardwareModel::Other);

			Ok(hardware_model)
		} else {
			Err(Error::new(
				std::io::ErrorKind::Other,
				format!(
					"Failed to get hardware model name: {}",
					String::from_utf8_lossy(&output.stderr)
				),
			))
		}
	}
	#[cfg(not(target_os = "macos"))]
	{
		Err(Error::new(
			std::io::ErrorKind::Unsupported,
			"Unsupported operating system",
		))
	}
}
