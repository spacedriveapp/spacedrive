//! # Power source
//!
//! Whether this device runs on battery, for background work that should
//! wait for mains power. A device that cannot tell, such as a desktop or a
//! platform without a reading here, counts as plugged in. The answer is
//! cached for half a minute, because asking can mean starting a process.
//!
//! ## Example
//! ```rust,no_run
//! # async fn example() {
//! use sd_core::ops::indexing::power::on_battery;
//!
//! while on_battery().await {
//!     tokio::time::sleep(std::time::Duration::from_secs(30)).await;
//! }
//! # }
//! ```

use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE: Duration = Duration::from_secs(30);

/// Whether the device is running on battery power now.
///
/// The reading runs off the async runtime, since it reads sysfs on Linux and
/// waits for `pmset` on macOS.
pub async fn on_battery() -> bool {
	static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
	let cached = *LAST.lock().unwrap_or_else(|e| e.into_inner());
	if let Some((at, value)) = cached {
		if at.elapsed() < CACHE {
			return value;
		}
	}
	let value = tokio::task::spawn_blocking(read).await.unwrap_or(false);
	*LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), value));
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
