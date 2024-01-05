//! Custom iNotify implementation for Location-Watcher for Android.
//! All because the notify-rs didn't want to work. D:

use inotify::{Event, EventMask, Inotify, WatchDescriptor, WatchMask};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{collections::HashMap, ffi::OsStr};

use thiserror::Error;
use tracing::{debug, info};

static mut RUNNING_WATCH_DESCRIPTORS: Option<
	Arc<Mutex<HashMap<String, WatchDescriptor>>>,
> = None;

fn init_running_watch_descriptors() {
	// Initialize the HashMap within the Arc<Mutex<>>
	let map = HashMap::new();
	unsafe {
		RUNNING_WATCH_DESCRIPTORS = Some(Arc::new(Mutex::new(map)));
	}
}

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

	init_running_watch_descriptors();

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

pub async fn add_watcher(
	inotify: &Inotify,
	directory_path: &str,
) -> Result<(), AndroidWatcherError> {
	let wd = inotify
		.watches()
		.add(
			directory_path,
			WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE,
		)
		.map_err(|_| AndroidWatcherError::AddWatch(directory_path.to_string()))?;

	info!("iNotify -> Watching directory: {}", directory_path);

	add_to_watcher_dict(directory_path.to_string().clone(), wd).await;

	Ok(())
}

pub async fn remove_watcher(
	inotify: &Inotify,
	directory_path: &str,
) -> Result<(), AndroidWatcherError> {
	let watcher = get_from_watcher_dict(directory_path.to_string().clone())
		.await
		.expect(AndroidWatcherError::FailedFindWatcher(directory_path.to_string().clone()).to_string().as_str());

	inotify.watches().remove(watcher).expect(AndroidWatcherError::RemoveWatch(directory_path.to_string()).to_string().as_str());

	remove_from_watcher_dict(directory_path.to_string().clone()).await;

	Ok(())
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

async fn add_to_watcher_dict(directory_path: String, watch_descriptor: WatchDescriptor) {
	unsafe {
		if let Some(ref map) = RUNNING_WATCH_DESCRIPTORS {
			let mut guard = map.lock().expect("Mutex lock poisoned");
			guard.insert(directory_path, watch_descriptor);
		}
	}
}

async fn remove_from_watcher_dict(directory_path: String) {
	unsafe {
		if let Some(ref map) = RUNNING_WATCH_DESCRIPTORS {
			let mut guard = map.lock().expect("Mutex lock poisoned");
			guard.remove(&directory_path);
		}
	}
}

async fn get_from_watcher_dict(directory_path: String) -> Option<WatchDescriptor> {
	unsafe {
		if let Some(ref map) = RUNNING_WATCH_DESCRIPTORS {
			let guard = map.lock().expect("Mutex lock poisoned");
			guard.get(&directory_path).cloned()
		} else {
			None
		}
	}
}
