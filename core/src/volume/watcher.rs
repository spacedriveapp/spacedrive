use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::{error, info};

use crate::volume::manager::VolumeManager;

#[cfg(target_os = "linux")]
use tokio_inotify::Inotify;

#[cfg(target_os = "macos")]
use fsevent::FSEventStream;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use winapi::um::winnt::FILE_NOTIFY_CHANGE_DISK_SPACE;

/// Spawns the appropriate watcher for the platform.
pub fn spawn_volume_watcher(manager: Arc<VolumeManager>) {
	let (sender, receiver) = mpsc::unbounded_channel();

	#[cfg(target_os = "linux")]
	tokio::spawn(linux_watcher(sender));

	#[cfg(target_os = "macos")]
	tokio::spawn(macos_watcher(sender));

	#[cfg(target_os = "windows")]
	tokio::spawn(windows_watcher(sender));

	tokio::spawn(volume_event_loop(manager, receiver));
}

/// Listens for volume events and triggers synchronization.
async fn volume_event_loop(
	manager: Arc<VolumeManager>,
	mut receiver: UnboundedReceiver<VolumeEvent>,
) {
	while let Some(event) = receiver.recv().await {
		match event {
			VolumeEvent::VolumeAdded => {
				info!("Volume added detected. Syncing...");
				manager.evaluate_system_volumes().await?;
			}
			VolumeEvent::VolumeRemoved => {
				info!("Volume removed detected. Syncing...");
				manager.evaluate_system_volumes().await?;
			}
			VolumeEvent::Error(err) => {
				error!("Volume watcher error: {}", err);
			}
		}
	}
}

/// Volume events to track.
pub enum VolumeEvent {
	VolumeAdded,
	VolumeRemoved,
	Error(String),
}

#[cfg(target_os = "linux")]
async fn linux_watcher(sender: UnboundedSender<VolumeEvent>) {
	let mut inotify = Inotify::init().expect("Failed to initialize inotify");

	// Monitor the /dev and /mnt directories for mount/unmount events.
	inotify
		.add_watch(
			"/dev",
			inotify::WatchMask::CREATE | inotify::WatchMask::DELETE,
		)
		.expect("Failed to add watch on /dev");
	inotify
		.add_watch(
			"/mnt",
			inotify::WatchMask::CREATE | inotify::WatchMask::DELETE,
		)
		.expect("Failed to add watch on /mnt");

	let mut buffer = [0; 1024];

	loop {
		let events = inotify
			.read_events_blocking(&mut buffer)
			.expect("Failed to read events");

		for event in events {
			if event.mask.contains(inotify::EventMask::CREATE) {
				sender
					.send(VolumeEvent::VolumeAdded)
					.expect("Failed to send event");
			} else if event.mask.contains(inotify::EventMask::DELETE) {
				sender
					.send(VolumeEvent::VolumeRemoved)
					.expect("Failed to send event");
			}
		}
	}
}

#[cfg(target_os = "macos")]
async fn macos_watcher(sender: UnboundedSender<VolumeEvent>) {
	let mut stream = FSEventStream::new(vec!["/Volumes".to_string()]);
	stream.start().expect("Failed to start FSEventStream");

	loop {
		match stream.next().await {
			Some(event) => {
				if event.flag.contains(fsevent::EventFlag::MOUNT) {
					sender
						.send(VolumeEvent::VolumeAdded)
						.expect("Failed to send event");
				} else if event.flag.contains(fsevent::EventFlag::UNMOUNT) {
					sender
						.send(VolumeEvent::VolumeRemoved)
						.expect("Failed to send event");
				}
			}
			None => error!("FSEventStream error occurred"),
		}
	}
}

#[cfg(target_os = "windows")]
async fn windows_watcher(sender: UnboundedSender<VolumeEvent>) {
	use std::ptr;
	use tokio::task;

	let path = std::ffi::OsString::from("C:\\")
		.encode_wide()
		.chain(Some(0))
		.collect::<Vec<_>>();

	unsafe {
		let handle = winapi::um::fileapi::CreateFileW(
			path.as_ptr(),
			winapi::um::winnt::FILE_LIST_DIRECTORY,
			winapi::um::winnt::FILE_SHARE_READ
				| winapi::um::winnt::FILE_SHARE_WRITE
				| winapi::um::winnt::FILE_SHARE_DELETE,
			ptr::null_mut(),
			winapi::um::fileapi::OPEN_EXISTING,
			winapi::um::winbase::FILE_FLAG_BACKUP_SEMANTICS,
			ptr::null_mut(),
		);

		if handle == winapi::um::handleapi::INVALID_HANDLE_VALUE {
			error!("Failed to open directory for watching");
			return;
		}

		task::spawn_blocking(move || {
			let mut buffer = [0u8; 1024];
			loop {
				let result = winapi::um::fileapi::ReadDirectoryChangesW(
					handle,
					buffer.as_mut_ptr() as *mut _,
					buffer.len() as u32,
					1, // Watch subtree
					FILE_NOTIFY_CHANGE_DISK_SPACE,
					ptr::null_mut(),
					ptr::null_mut(),
					None,
				);

				if result == 0 {
					error!("ReadDirectoryChangesW failed");
					return;
				}

				sender
					.send(VolumeEvent::VolumeAdded)
					.expect("Failed to send event");
			}
		});
	}
}
