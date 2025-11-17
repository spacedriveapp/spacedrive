use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse, EventFilter};
use crate::infra::event::log_emitter::{set_global_log_bus, LogMessage};
use crate::infra::event::{Event, EventSubscriber};
use crate::Core;

/// Connection information for event streaming
#[derive(Debug)]
struct Connection {
	id: Uuid,
	event_tx: mpsc::UnboundedSender<Event>,
	event_types: Vec<String>,
	filter: Option<EventFilter>,
}

/// Minimal JSON-over-UDS RPC server with event streaming support
pub struct RpcServer {
	socket_path: PathBuf,
	core: Arc<Core>,
	shutdown_tx: mpsc::Sender<()>,
	shutdown_rx: mpsc::Receiver<()>,
	/// Active connections for event streaming
	connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
	/// Connection counter for monitoring
	connection_count: Arc<AtomicUsize>,
	/// Maximum number of concurrent connections
	max_connections: usize,
}

impl RpcServer {
	pub fn new(socket_path: PathBuf, core: Arc<Core>) -> Self {
		let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
		Self {
			socket_path,
			core,
			shutdown_tx,
			shutdown_rx,
			connections: Arc::new(RwLock::new(HashMap::new())),
			connection_count: Arc::new(AtomicUsize::new(0)),
			max_connections: 100, // Reasonable limit for concurrent connections
		}
	}

	pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		tracing::info!("Starting RPC server...");
		if std::fs::remove_file(&self.socket_path).is_ok() {}
		if let Some(parent) = self.socket_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let listener = UnixListener::bind(&self.socket_path)?;
		tracing::info!("RPC server bound to socket: {:?}", self.socket_path);

		// Start event broadcaster
		tracing::info!("Starting event broadcaster...");
		self.start_event_broadcaster().await?;
		tracing::info!("Event broadcaster started successfully");

		loop {
			tokio::select! {
				// Handle new connections
				result = listener.accept() => {
					match result {
						Ok((mut stream, _addr)) => {
							// Check connection limit
							let current_connections = self.connection_count.load(Ordering::Relaxed);
							if current_connections >= self.max_connections {
								tracing::warn!(
									"Connection limit reached ({}), rejecting new connection",
									self.max_connections
								);
								// Close the stream immediately to free the file descriptor
								let _ = stream.shutdown().await;
								continue;
							}

							let core = self.core.clone();
							let shutdown_tx = self.shutdown_tx.clone();
							let connections = self.connections.clone();
							let connection_count = self.connection_count.clone();

							// Increment connection counter
							connection_count.fetch_add(1, Ordering::Relaxed);

							// Spawn task for concurrent request handling
							tokio::spawn(async move {
								// Convert errors to strings to ensure Send
								if let Err(e) = Self::handle_connection(stream, core, shutdown_tx, connections, connection_count).await {
									eprintln!("Connection error: {}", e);
								}
							});
						}
						Err(e) => {
							// Handle specific "too many open files" error
							if e.raw_os_error() == Some(24) {
								tracing::error!("Too many open files error (EMFILE) - system file descriptor limit reached");
								tracing::error!("Current connections: {}", self.connection_count.load(Ordering::Relaxed));
								tracing::error!("Consider increasing system limits or reducing concurrent connections");
							} else {
								eprintln!("Accept error: {}", e);
							}
							continue;
						}
					}
				}

				// Handle shutdown signal
				_ = self.shutdown_rx.recv() => {
					eprintln!("Shutdown signal received, stopping RPC server");

					// Perform graceful shutdown of the core
					tracing::info!("Performing graceful core shutdown...");
					if let Err(e) = self.core.shutdown().await {
						tracing::error!("Error during core shutdown: {}", e);
					}

					break;
				}
			}
		}

		Ok(())
	}

	/// Start the event broadcaster that forwards core events to subscribed connections
	async fn start_event_broadcaster(&self) -> Result<(), Box<dyn std::error::Error>> {
		let core = self.core.clone();

		// Make the core's LogBus globally available to the LogEventLayer
		set_global_log_bus(core.logs.clone());
		tracing::info!("Log bus registered for realtime streaming");

		// Start main event broadcaster
		let mut event_subscriber = core.events.subscribe();
		let connections = self.connections.clone();

		tokio::spawn(async move {
			while let Ok(event) = event_subscriber.recv().await {
				let connections_read = connections.read().await;

				// Broadcast event to all subscribed connections
				for connection in connections_read.values() {
					if Self::should_forward_event(
						&event,
						&connection.event_types,
						&connection.filter,
					) {
						// Ignore errors if connection is closed
						let _ = connection.event_tx.send(event.clone());
					}
				}
			}
		});

		// Start separate log message broadcaster
		let mut log_subscriber = core.logs.subscribe();
		let connections_for_logs = self.connections.clone();

		tokio::spawn(async move {
			while let Ok(log_msg) = log_subscriber.recv().await {
				let connections_read = connections_for_logs.read().await;

				// Convert LogMessage to Event for transport compatibility
				let event = Event::LogMessage {
					timestamp: log_msg.timestamp,
					level: log_msg.level,
					target: log_msg.target,
					message: log_msg.message,
					job_id: log_msg.job_id,
					library_id: log_msg.library_id,
				};

				// Broadcast to connections that subscribed to LogMessage events
				for connection in connections_read.values() {
					if Self::should_forward_event(
						&event,
						&connection.event_types,
						&connection.filter,
					) {
						// Ignore errors if connection is closed
						let _ = connection.event_tx.send(event.clone());
					}
				}
			}
		});

		Ok(())
	}

	/// Execute a JSON operation using the registry handlers
	///
	/// Made public for reuse in embedded implementations (iOS, etc.)
	pub async fn execute_json_operation(
		method: &str,
		library_id: Option<uuid::Uuid>,
		json_payload: serde_json::Value,
		core: &Arc<crate::Core>,
	) -> Result<serde_json::Value, String> {
		tracing::info!(
			"[RPC Operation]: method={}, library_id={:?}",
			method,
			library_id
		);
		// Create base session context
		let base_session = core.api_dispatcher.create_base_session()?;

		// Try library queries first
		if let Some(handler) = crate::infra::wire::registry::LIBRARY_QUERIES.get(method) {
			let library_id =
				library_id.ok_or_else(|| "Library ID required for library query".to_string())?;
			let session = base_session.with_library(library_id);
			return handler(core.context.clone(), session, json_payload).await;
		}

		// Try core queries
		if let Some(handler) = crate::infra::wire::registry::CORE_QUERIES.get(method) {
			return handler(core.context.clone(), base_session, json_payload).await;
		}

		// Try library actions
		if let Some(handler) = crate::infra::wire::registry::LIBRARY_ACTIONS.get(method) {
			let library_id =
				library_id.ok_or_else(|| "Library ID required for library action".to_string())?;
			let session = base_session.with_library(library_id);
			return handler(core.context.clone(), session, json_payload).await;
		}

		// Try core actions
		if let Some(handler) = crate::infra::wire::registry::CORE_ACTIONS.get(method) {
			return handler(core.context.clone(), json_payload).await;
		}

		Err(format!("Unknown method: {}", method))
	}

	/// Check if an event should be forwarded to a connection based on filters
	fn should_forward_event(
		event: &Event,
		event_types: &[String],
		filter: &Option<EventFilter>,
	) -> bool {
		// If event_types is empty, forward all events
		// Otherwise, treat event_types as an INCLUSION list (only forward these)
		if !event_types.is_empty() {
			// Use the Event's own variant_name() method - single source of truth!
			let event_type = event.variant_name();

			if !event_types.contains(&event_type.to_string()) {
				return false;
			}
		}

		// Apply additional filters if specified
		if let Some(filter) = filter {
			// Filter by resource type
			if let Some(filter_resource_type) = &filter.resource_type {
				if let Some(event_resource_type) = event.resource_type() {
					if event_resource_type != filter_resource_type {
						return false;
					}
				} else {
					// Event is not a resource event, but filter expects one
					return false;
				}
			}

			// Filter by path scope (for resource events)
			if let Some(path_scope) = &filter.path_scope {
				if !event.affects_path(path_scope) {
					return false;
				}
			}

			match event {
				Event::JobProgress { job_id, .. }
				| Event::JobStarted { job_id, .. }
				| Event::JobCompleted { job_id, .. }
				| Event::JobFailed { job_id, .. }
				| Event::JobCancelled { job_id, .. } => {
					if let Some(filter_job_id) = &filter.job_id {
						return job_id == filter_job_id;
					}
				}
				Event::LibraryCreated { id, .. }
				| Event::LibraryOpened { id, .. }
				| Event::LibraryClosed { id, .. } => {
					if let Some(filter_library_id) = &filter.library_id {
						return id == filter_library_id;
					}
				}
				Event::LogMessage {
					job_id, library_id, ..
				} => {
					// Filter by job ID if specified
					if let Some(filter_job_id) = &filter.job_id {
						if let Some(log_job_id) = job_id {
							return log_job_id == filter_job_id;
						} else {
							return false; // No job ID in log, but filter requires one
						}
					}

					// Filter by library ID if specified
					if let Some(filter_library_id) = &filter.library_id {
						if let Some(log_library_id) = library_id {
							return log_library_id == filter_library_id;
						} else {
							return false; // No library ID in log, but filter requires one
						}
					}
				}
				_ => {}
			}
		}

		true
	}

	/// Handle individual client connection concurrently
	async fn handle_connection(
		stream: tokio::net::UnixStream,
		core: Arc<Core>,
		shutdown_tx: mpsc::Sender<()>,
		connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
		connection_count: Arc<AtomicUsize>,
	) -> Result<(), String> {
		let connection_id = Uuid::new_v4();
		let (mut reader, mut writer) = stream.into_split();
		let mut buf_reader = BufReader::new(reader);
		let mut line = String::new();

		// Channel for sending events to this connection
		let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

		loop {
			tokio::select! {
				// Handle incoming requests from client
				result = buf_reader.read_line(&mut line) => {
					match result {
						Ok(0) => {
							// EOF - client closed connection
							break;
						}
						Ok(_) => {
							// Parse request
							match serde_json::from_str::<DaemonRequest>(&line.trim()) {
								Ok(request) => {
									let response = Self::process_request(
										request,
										&core,
										&shutdown_tx,
										&connections,
										connection_id,
										&event_tx
									).await;

									// Send response
									let response_json = serde_json::to_string(&response)
										.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;

									if let Err(_) = writer.write_all((response_json + "\n").as_bytes()).await {
										break; // Connection closed
									}

									// For non-streaming requests, close connection after response
									match response {
										DaemonResponse::Subscribed => {
											// Keep connection open for streaming
										}
										DaemonResponse::Unsubscribed => {
											// Close connection after unsubscribe
											break;
										}
										DaemonResponse::Event(_) => {
											// This shouldn't happen in request processing
										}
										_ => {
											// Regular request-response, close connection
											break;
										}
									}
								}
								Err(e) => {
									tracing::error!("Failed to parse daemon request: {}", e);
									tracing::error!("Raw request: {}", line.trim());
									// Send error response
									let error_response = DaemonResponse::Error(DaemonError::SerializationError(e.to_string()));
									let response_json = serde_json::to_string(&error_response)
										.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;
									if let Err(_) = writer.write_all((response_json + "\n").as_bytes()).await {
										break; // Connection closed
									}
									break; // Close connection after error
								}
							}
							line.clear();
						}
						Err(_) => break, // Connection error
					}
				}

				// Handle outgoing events to client
				Some(event) = event_rx.recv() => {
					let response = DaemonResponse::Event(event);
					let response_json = serde_json::to_string(&response)
						.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;

					if let Err(_) = writer.write_all((response_json + "\n").as_bytes()).await {
						break; // Connection closed
					}
				}
			}
		}

		// Clean up connection
		connections.write().await.remove(&connection_id);

		// Decrement connection counter
		connection_count.fetch_sub(1, Ordering::Relaxed);

		Ok(())
	}

	/// Process a parsed daemon request
	async fn process_request(
		request: DaemonRequest,
		core: &Arc<Core>,
		shutdown_tx: &mpsc::Sender<()>,
		connections: &Arc<RwLock<HashMap<Uuid, Connection>>>,
		connection_id: Uuid,
		event_tx: &mpsc::UnboundedSender<Event>,
	) -> DaemonResponse {
		match request {
			DaemonRequest::Ping => DaemonResponse::Pong,

			DaemonRequest::Action {
				method,
				library_id,
				payload,
			} => {
				// Handle JSON actions with direct JSON-to-JSON processing
				match Self::execute_json_operation(&method, library_id, payload, core).await {
					Ok(json_result) => DaemonResponse::JsonOk(json_result),
					Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
				}
			}

			DaemonRequest::Query {
				method,
				library_id,
				payload,
			} => {
				// Handle JSON queries with direct JSON-to-JSON processing
				match Self::execute_json_operation(&method, library_id, payload, core).await {
					Ok(json_result) => DaemonResponse::JsonOk(json_result),
					Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
				}
			}

			DaemonRequest::Subscribe {
				event_types,
				filter,
			} => {
				// Register connection for event streaming
				let connection = Connection {
					id: connection_id,
					event_tx: event_tx.clone(),
					event_types,
					filter,
				};

				connections.write().await.insert(connection_id, connection);
				DaemonResponse::Subscribed
			}

			DaemonRequest::Unsubscribe => {
				// Remove connection from event streaming
				connections.write().await.remove(&connection_id);
				DaemonResponse::Unsubscribed
			}

			DaemonRequest::Shutdown => {
				// Signal shutdown to main loop
				let _ = shutdown_tx.send(()).await;
				DaemonResponse::Ok(Vec::new())
			}
		}
	}

	/// Get current connection statistics
	pub fn get_connection_stats(&self) -> (usize, usize) {
		let current = self.connection_count.load(Ordering::Relaxed);
		let max = self.max_connections;
		(current, max)
	}
}
