use std::io::Error;
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
	Simulator,
	Android,
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
	#[cfg(target_os = "ios")]
	{
		use std::ffi::CString;
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
			let hardware_model = HardwareModel::from_display_name(&device_type.as_str());

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
		Ok(HardwareModel::Android)
	}

	#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
	{
		Err(Error::new(
			std::io::ErrorKind::Unsupported,
			"Unsupported operating system",
		))
	}
}
