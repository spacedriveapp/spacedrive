use std::io;

use serde::{Deserialize, Serialize};
use specta::Type;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

#[repr(i32)]
#[derive(Debug, Clone, Display, Copy, EnumIter, Type, Serialize, Deserialize, Eq, PartialEq)]
#[specta(rename = "CoreHardwareModel")]
pub enum HardwareModel {
	Other = 0,
	MacStudio = 1,
	MacBookAir = 2,
	MacBookPro = 3,
	MacBook = 4,
	MacMini = 5,
	MacPro = 6,
	IMac = 7,
	IMacPro = 8,
	IPad = 9,
	IPhone = 10,
	Simulator = 11,
	Android = 12,
}

impl From<i32> for HardwareModel {
	fn from(value: i32) -> Self {
		match value {
			1 => Self::MacStudio,
			2 => Self::MacBookAir,
			3 => Self::MacBookPro,
			4 => Self::MacBook,
			5 => Self::MacMini,
			6 => Self::MacPro,
			7 => Self::IMac,
			8 => Self::IMacPro,
			9 => Self::IPad,
			10 => Self::IPhone,
			11 => Self::Simulator,
			12 => Self::Android,
			_ => Self::Other,
		}
	}
}

impl From<HardwareModel> for sd_cloud_schema::devices::HardwareModel {
	fn from(model: HardwareModel) -> Self {
		match model {
			HardwareModel::MacStudio => Self::MacStudio,
			HardwareModel::MacBookAir => Self::MacBookAir,
			HardwareModel::MacBookPro => Self::MacBookPro,
			HardwareModel::MacBook => Self::MacBook,
			HardwareModel::MacMini => Self::MacMini,
			HardwareModel::MacPro => Self::MacPro,
			HardwareModel::IMac => Self::IMac,
			HardwareModel::IMacPro => Self::IMacPro,
			HardwareModel::IPad => Self::IPad,
			HardwareModel::IPhone => Self::IPhone,
			HardwareModel::Simulator => Self::Simulator,
			HardwareModel::Android => Self::Android,
			HardwareModel::Other => Self::Other,
		}
	}
}

impl From<sd_cloud_schema::devices::HardwareModel> for HardwareModel {
	fn from(model: sd_cloud_schema::devices::HardwareModel) -> Self {
		match model {
			sd_cloud_schema::devices::HardwareModel::MacStudio => Self::MacStudio,
			sd_cloud_schema::devices::HardwareModel::MacBookAir => Self::MacBookAir,
			sd_cloud_schema::devices::HardwareModel::MacBookPro => Self::MacBookPro,
			sd_cloud_schema::devices::HardwareModel::MacBook => Self::MacBook,
			sd_cloud_schema::devices::HardwareModel::MacMini => Self::MacMini,
			sd_cloud_schema::devices::HardwareModel::MacPro => Self::MacPro,
			sd_cloud_schema::devices::HardwareModel::IMac => Self::IMac,
			sd_cloud_schema::devices::HardwareModel::IMacPro => Self::IMacPro,
			sd_cloud_schema::devices::HardwareModel::IPad => Self::IPad,
			sd_cloud_schema::devices::HardwareModel::IPhone => Self::IPhone,
			sd_cloud_schema::devices::HardwareModel::Simulator => Self::Simulator,
			sd_cloud_schema::devices::HardwareModel::Android => Self::Android,
			sd_cloud_schema::devices::HardwareModel::Other => Self::Other,
		}
	}
}

impl From<&str> for HardwareModel {
	fn from(name: &str) -> Self {
		Self::iter()
			.find(|&model| {
				model.to_string().to_lowercase().replace(' ', "")
					== name.to_lowercase().replace(' ', "")
			})
			.unwrap_or(Self::Other)
	}
}

impl HardwareModel {
	pub fn try_get() -> Result<Self, io::Error> {
		#[cfg(target_os = "macos")]
		{
			use std::process::Command;

			let output = Command::new("system_profiler")
				.arg("SPHardwareDataType")
				.output()?;

			if output.status.success() {
				let output_str = std::str::from_utf8(&output.stdout).unwrap_or_default();
				let hardware_model = output_str
					.lines()
					.find(|line| line.to_lowercase().contains("model name"))
					.and_then(|line| line.split_once(':'))
					.map(|(_, model_name)| model_name.trim().into())
					.unwrap_or(Self::Other);

				Ok(hardware_model)
			} else {
				Err(io::Error::new(
					io::ErrorKind::Other,
					format!(
						"Failed to get hardware model name: {}",
						String::from_utf8_lossy(&output.stderr)
					),
				))
			}
		}
		#[cfg(target_os = "ios")]
		{
			use std::ffi::CString;
			use std::io::Error;
			use std::ptr;

			extern "C" {
				fn sysctlbyname(
					name: *const libc::c_char,
					oldp: *mut libc::c_void,
					oldlenp: *mut usize,
					newp: *mut libc::c_void,
					newlen: usize,
				) -> libc::c_int;
			}

			fn get_device_type() -> Option<String> {
				let mut size: usize = 0;
				let name = CString::new("hw.machine").expect("CString::new failed");

				// First, get the size of the buffer needed
				unsafe {
					sysctlbyname(
						name.as_ptr(),
						ptr::null_mut(),
						&mut size,
						ptr::null_mut(),
						0,
					);
				}

				// Allocate a buffer with the correct size
				let mut buffer: Vec<u8> = vec![0; size];

				// Get the actual machine type
				unsafe {
					sysctlbyname(
						name.as_ptr(),
						buffer.as_mut_ptr() as *mut libc::c_void,
						&mut size,
						ptr::null_mut(),
						0,
					);
				}

				// Convert the buffer to a String
				let machine_type = String::from_utf8_lossy(&buffer).trim().to_string();

				// Check if the device is an iPad or iPhone
				if machine_type.starts_with("iPad") {
					Some("iPad".to_string())
				} else if machine_type.starts_with("iPhone") {
					Some("iPhone".to_string())
				} else if machine_type.starts_with("arm") {
					Some("Simulator".to_string())
				} else {
					None
				}
			}

			if let Some(device_type) = get_device_type() {
				let hardware_model = HardwareModel::from(device_type.as_str());

				Ok(hardware_model)
			} else {
				Err(Error::new(
					std::io::ErrorKind::Other,
					"Failed to get hardware model name",
				))
			}
		}

		#[cfg(target_os = "android")]
		{
			Ok(Self::Android)
		}

		#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
		{
			Ok(Self::Other)
		}
	}
}
