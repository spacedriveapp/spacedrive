//! Custom iNotify implementation for Location-Watcher for Android.
//! All because the notify-rs didn't want to work. D:

use inotify::{Event, EventMask, Inotify, WatchDescriptor, WatchMask};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{collections::HashMap, ffi::OsStr};

use thiserror::Error;
use tracing::{debug, info};
use once_cell::sync::Lazy;

pub static RUNNING_WATCH_DESCRIPTORS: Lazy<Arc<Mutex<HashMap<&'static str, WatchDescriptor>>>> = Lazy::new(|| {
	Arc::new(Mutex::new(HashMap::new()))
});

fn handle_event(event: &Event<&OsStr>) {
	info!("Received event: {:?}", event);

	// Add your logic here to handle different event types
	match event.mask {
		EventMask::CREATE => {
			debug!("File or directory created: {:?}", event.name);
			// Add more handling logic as needed
		}
		EventMask::MODIFY => {
			debug!("File or directory modified: {:?}", event.name);
			// Add more handling logic as needed
		}
		EventMask::DELETE => {
			debug!("File or directory deleted: {:?}", event.name);
			// Add more handling logic as needed
		}
		_ => {
			debug!("Other event: {:?}", event);
			// Add more handling logic for other event types
		}
	}
}

pub fn watch_directory(directory_path: &str) {
	let mut inotify = Inotify::init().expect("Failed to initialize inotify");

	// Add a watch to the specified directory
	inotify
		.watches()
		.add(
			directory_path,
			WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE,
		)
		.expect("Failed to add watch");

	info!("iNotify -> Watching directory: {}", directory_path);

	// Start watching events in the main thread
	let mut buffer = [0u8; 4096];

	loop {
		// Read events with a timeout to avoid blocking indefinitely
		if let Ok(events) = inotify.read_events(&mut buffer) {
			for event in events {
				handle_event(&event);
			}
		}

		// Introduce a short delay to avoid busy-waiting and reduce CPU usage
		std::thread::sleep(Duration::from_secs(2));
	}
}

pub fn init() -> Inotify {
	let mut inotify = Inotify::init().expect("Failed to initialize inotify");

	run_event_watcher(&mut inotify);

	return inotify;
}

fn run_event_watcher(inotify: &mut Inotify) {
	let mut buffer = [0u8; 4096];

	loop {
		// Read events with a timeout to avoid blocking indefinitely
		if let Ok(events) = inotify.read_events(&mut buffer) {
			for event in events {
				handle_event(&event);
			}
		}

		// Introduce a short delay to avoid busy-waiting and reduce CPU usage
		std::thread::sleep(Duration::from_secs(2));
	}
}

pub fn add_watcher(inotify: &Inotify, directory_path: &str) -> Result<(), AndroidWatcherError> {
	lazy_static! {
		static ref RUNNING_WATCH_DESCRIPTORS: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
	}

	let wd = inotify
		.watches()
		.add(
			directory_path,
			WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE,
		)
		.map_err(|_| AndroidWatcherError::AddWatch(directory_path.to_string()))?;

	info!("iNotify -> Watching directory: {}", directory_path);

	let running_watch_descriptors = Arc::clone(&RUNNING_WATCH_DESCRIPTORS);

	let mut watch_descriptors = running_watch_descriptors.lock().unwrap();
	watch_descriptors.insert(directory_path.to_string(), wd);

	info!("iNotify -> Watcher has been added to RUNNING_WATCH_DESCRIPTORS");

	Ok(())
}

pub fn remove_watcher(inotify: &Inotify, directory_path: &str) -> Result<(), AndroidWatcherError> {
	// Fetch Watch Descriptor from RUNNING_WATCH_DESCRIPTORS
	let running_watch_descriptors = Arc::clone(unsafe { &RUNNING_WATCH_DESCRIPTORS });

	unsafe {
		let mut watch_descriptors = running_watch_descriptors.lock().unwrap();
		let wd = watch_descriptors.get(directory_path).expect(
			AndroidWatcherError::FailedFindWatcher(directory_path.to_string()).to_string().as_str()
		);

		inotify.watches().remove(wd.clone());

		watch_descriptors.remove(directory_path);

		Ok(())
	}
}

#[derive(Error, Debug)]
pub enum AndroidWatcherError {
	/// The provided path is not a directory.
	#[error("Watcher error: (error: {0})")]
	NotDirectory(#[from] std::io::Error),
	/// The provided path is not a valid UTF-8 string.
	#[error("Invalid path: (path: {0})")]
	InvalidPath(#[from] std::str::Utf8Error),
	/// Failed to add watch.
	#[error("Failed to add watch: (error: {0})")]
	AddWatch(String),
	/// Failed to remove watch.
	#[error("Failed to remove watch: (error: {0})")]
	RemoveWatch(String),
	#[error("Failed to find Path in RUNNING_WATCH_DESCRIPTORS: (error: {0})")]
	FailedFindWatcher(String),
}
