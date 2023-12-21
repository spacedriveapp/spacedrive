use crate::{prisma::location, library::Library, Node};
use inotify::{Event, EventMask, Inotify, WatchMask};
use std::{ffi::OsStr, sync::Arc};
use std::time::Duration;

use tracing::{debug, info};

use super::LocationManagerError;

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
    // Create an inotify instance
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");

    // Add a watch to the specified directory
    inotify
        .watches()
        .add(directory_path, WatchMask::CREATE | WatchMask::MODIFY | WatchMask::DELETE)
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