use std::io::BufRead;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::invalidate_query;
use crate::library::Library;

/// Currently the only thing we do is invalidate the volumes.list query.
/// Later, we will want to extract specific data into a struct.
/// That way we can determine if we want to trigger the import files flow.
///
fn handle_disk_change(library: Arc<Library>) {
	// Clone the Arc to be moved into the closure
	let library_cloned = library.clone();

	// Spawn a new thread to perform a delayed operation
	thread::spawn(move || {
		thread::sleep(Duration::from_millis(500)); // Delay for 500 milliseconds
		invalidate_query!(library_cloned, "volumes.list");
	});
}

pub fn spawn_volume_watcher(library: Arc<Library>) {
	#[cfg(target_os = "macos")]
	thread::spawn(move || {
		let mut child = Command::new("diskutil")
			.arg("activity")
			.stdout(std::process::Stdio::piped())
			.spawn()
			.expect("Failed to start diskutil");

		let stdout = child.stdout.as_mut().expect("Failed to capture stdout");
		let mut reader = std::io::BufReader::new(stdout);

		let mut buffer = String::new();
		while reader.read_line(&mut buffer).expect("Failed to read line") > 0 {
			if buffer.contains("DiskAppeared") || buffer.contains("DiskDisappeared") {
				// println!("Disk change detected: {:?}", &buffer);
				handle_disk_change(library.clone());
			}
			buffer.clear();
		}
	});

	#[cfg(target_os = "linux")]
	thread::spawn(move || {
		let mut child = Command::new("udevadm")
			.arg("monitor")
			.stdout(std::process::Stdio::piped())
			.spawn()
			.expect("Failed to start udevadm");

		let stdout = child.stdout.as_mut().expect("Failed to capture stdout");
		let mut reader = std::io::BufReader::new(stdout);

		let mut buffer = String::new();
		while reader.read_line(&mut buffer).expect("Failed to read line") > 0 {
			if buffer.contains("add") || buffer.contains("remove") {
				println!("Disk change detected: {:?}", &buffer);
				handle_disk_change(library.clone());
			}

			buffer.clear();
		}
	});

	#[cfg(target_os = "windows")]
	thread::spawn(move || {
		let mut child = Command::new("wmic")
			.arg("diskdrive")
			.stdout(std::process::Stdio::piped())
			.spawn()
			.expect("Failed to start wmic");

		// Shared handling code
		// ...
		// handle_disk_change(library.clone());
	});
}
