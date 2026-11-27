//! # Power source
//!
//! Whether this device runs on battery, for background work that should
//! wait for mains power. A device that cannot tell, such as a desktop or a
//! platform without a reading here, counts as plugged in. The answer is
//! cached for half a minute, because asking can mean starting a process.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE: Duration = Duration::from_secs(30);

/// Whether the device is running on battery power now.
pub fn on_battery() -> bool {
	static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
	let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
	if let Some((at, value)) = *last {
		if at.elapsed() < CACHE {
			return value;
		}
	}
	let value = read();
	*last = Some((Instant::now(), value));
	value
}

#[cfg(target_os = "linux")]
fn read() -> bool {
	let Ok(supplies) = std::fs::read_dir("/sys/class/power_supply") else {
		return false;
	};
	let field = |dir: &std::path::Path, name: &str| {
		std::fs::read_to_string(dir.join(name))
			.map(|s| s.trim().to_string())
			.unwrap_or_default()
	};
	let mut discharging = false;
	for supply in supplies.flatten() {
		let dir = supply.path();
		match field(&dir, "type").as_str() {
			"Mains" | "USB" if field(&dir, "online") == "1" => return false,
			"Battery" if field(&dir, "status") == "Discharging" => discharging = true,
			_ => {}
		}
	}
	discharging
}

#[cfg(target_os = "macos")]
fn read() -> bool {
	std::process::Command::new("/usr/bin/pmset")
		.args(["-g", "batt"])
		.output()
		.map(|out| String::from_utf8_lossy(&out.stdout).contains("'Battery Power'"))
		.unwrap_or(false)
}

#[cfg(windows)]
fn read() -> bool {
	use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
	// SAFETY: GetSystemPowerStatus writes the struct it is given and nothing else.
	unsafe {
		let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
		GetSystemPowerStatus(&mut status) != 0 && status.ACLineStatus == 0
	}
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn read() -> bool {
	false
}
