//! Library statistics listener service
//!
//! Manages per-library statistics recalculation listeners that respond to resource changes

mod listener;

use crate::{context::CoreContext, infra::event::Event, service::Service};
use anyhow::Result;
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, RwLock,
	},
};
use tokio::{sync::broadcast::error::RecvError, task::JoinHandle};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Service that manages statistics listeners for all libraries
pub struct StatisticsListenerService {
	context: Arc<CoreContext>,
	running: AtomicBool,
	monitor_handle: RwLock<Option<JoinHandle<()>>>,
	listeners: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
}

impl StatisticsListenerService {
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self {
			context,
			running: AtomicBool::new(false),
			monitor_handle: RwLock::new(None),
			listeners: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Monitor loop that watches for library lifecycle events and manages listeners
	async fn monitor_loop(
		context: Arc<CoreContext>,
		running: Arc<AtomicBool>,
		listeners: Arc<RwLock<HashMap<Uuid, JoinHandle<()>>>>,
	) {
		info!("Statistics listener service monitor started");

		let mut event_rx = context.events.subscribe();

		while running.load(Ordering::SeqCst) {
			match event_rx.recv().await {
				Ok(Event::LibraryOpened { id, name, .. }) => {
					debug!(library_id = %id, library_name = %name, "Library opened, starting statistics listener");

					// Get the library from context
					let library_manager = context.libraries().await;
					if let Some(library) = library_manager.get_library(id).await {
						// Spawn listener for this library
						let handle =
							listener::spawn_statistics_listener(library, context.events.clone());

						// Store the handle
						listeners.write().unwrap().insert(id, handle);
						info!(library_id = %id, library_name = %name, "Statistics listener started for library");
					} else {
						warn!(library_id = %id, "Library opened event received but library not found in manager");
					}
				}
				Ok(Event::LibraryClosed { id, .. }) => {
					debug!(library_id = %id, "Library closed, stopping statistics listener");

					// Remove and abort the listener task
					if let Some(handle) = listeners.write().unwrap().remove(&id) {
						handle.abort();
						info!(library_id = %id, "Statistics listener stopped for library");
					}
				}
				Err(RecvError::Lagged(skipped)) => {
					warn!(
						skipped = skipped,
						"Statistics listener service event subscriber lagged"
					);
				}
				Err(RecvError::Closed) => {
					info!("Event bus closed, statistics listener service monitor shutting down");
					break;
				}
				_ => {
					// Ignore other events
				}
			}
		}

		// Clean up all listeners on shutdown
		let mut listeners = listeners.write().unwrap();
		for (library_id, handle) in listeners.drain() {
			handle.abort();
			debug!(library_id = %library_id, "Aborted statistics listener during shutdown");
		}

		info!("Statistics listener service monitor stopped");
	}

	/// Start listeners for all currently opened libraries
	async fn start_existing_listeners(&self) {
		let library_manager = self.context.libraries().await;
		let libraries = library_manager.get_open_libraries().await;

		for library in libraries {
			let library_id = library.id();
			let library_name = library.name().await;

			debug!(library_id = %library_id, library_name = %library_name, "Starting statistics listener for existing library");

			// Spawn listener
			let handle = listener::spawn_statistics_listener(library, self.context.events.clone());

			// Store the handle
			self.listeners.write().unwrap().insert(library_id, handle);
			info!(library_id = %library_id, library_name = %library_name, "Statistics listener started for existing library");
		}
	}
}

#[async_trait::async_trait]
impl Service for StatisticsListenerService {
	async fn start(&self) -> Result<()> {
		if self.running.swap(true, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Starting statistics listener service");

		// Start listeners for all currently opened libraries
		self.start_existing_listeners().await;

		// Spawn monitor loop to watch for new libraries
		let running = Arc::new(AtomicBool::new(true));

		let handle = tokio::spawn(Self::monitor_loop(
			self.context.clone(),
			running.clone(),
			self.listeners.clone(),
		));

		*self.monitor_handle.write().unwrap() = Some(handle);

		info!("Statistics listener service started");
		Ok(())
	}

	async fn stop(&self) -> Result<()> {
		if !self.running.swap(false, Ordering::SeqCst) {
			return Ok(());
		}

		info!("Stopping statistics listener service");

		// Stop monitor loop
		if let Some(handle) = self.monitor_handle.write().unwrap().take() {
			handle.abort();
		}

		// Stop all library listeners
		let mut listeners = self.listeners.write().unwrap();
		for (library_id, handle) in listeners.drain() {
			handle.abort();
			debug!(library_id = %library_id, "Stopped statistics listener");
		}

		info!("Statistics listener service stopped");
		Ok(())
	}

	fn is_running(&self) -> bool {
		self.running.load(Ordering::SeqCst)
	}

	fn name(&self) -> &'static str {
		"statistics_listener"
	}
}
