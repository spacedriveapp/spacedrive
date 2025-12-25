use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::infra::daemon::event_buffer::EventBuffer;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse, EventFilter};
use crate::infra::event::log_emitter::{set_global_log_bus, LogMessage};
use crate::infra::event::{Event, EventSubscriber};
use crate::Core;

/// Connection information for event and log streaming
#[derive(Debug)]
struct Connection {
	id: Uuid,
	response_tx: mpsc::UnboundedSender<DaemonResponse>,
	event_types: Vec<String>,
	filter: Option<EventFilter>,
	log_filter: Option<crate::infra::daemon::types::LogFilter>,
}

/// Minimal JSON-over-TCP RPC server with event streaming support
pub struct RpcServer {
	socket_addr: String,
	core: Arc<Core>,
	shutdown_tx: mpsc::Sender<()>,
	shutdown_rx: mpsc::Receiver<()>,
	/// Active connections for event streaming
	connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
	/// Connection counter for monitoring
	connection_count: Arc<AtomicUsize>,
	/// Maximum number of concurrent connections
	max_connections: usize,
	/// Event buffer for handling subscription race conditions
	event_buffer: Arc<EventBuffer>,
}

impl RpcServer {
	pub fn new(socket_addr: String, core: Arc<Core>) -> Self {
		let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
		Self {
			socket_addr,
			core,
			shutdown_tx,
			shutdown_rx,
			connections: Arc::new(RwLock::new(HashMap::new())),
			connection_count: Arc::new(AtomicUsize::new(0)),
			max_connections: 100, // Reasonable limit for concurrent connections
			event_buffer: Arc::new(EventBuffer::new()),
		}
	}

	pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		tracing::info!("Starting RPC server...");
		let listener = TcpListener::bind(&self.socket_addr).await?;
		tracing::info!("RPC server bound to: {}", self.socket_addr);

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
							let event_buffer = self.event_buffer.clone();

							// Increment connection counter
							connection_count.fetch_add(1, Ordering::Relaxed);

							// Spawn task for concurrent request handling
							tokio::spawn(async move {
								// Convert errors to strings to ensure Send
								if let Err(e) = Self::handle_connection(stream, core, shutdown_tx, connections, connection_count, event_buffer).await {
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
		let event_buffer = self.event_buffer.clone();

		tokio::spawn(async move {
			while let Ok(event) = event_subscriber.recv().await {
				// Add to buffer before broadcasting (for subscription race condition handling)
				event_buffer.add_event(event.clone()).await;

				let connections_read = connections.read().await;

				// Broadcast event to all subscribed connections
				for connection in connections_read.values() {
					let should_forward = Self::should_forward_event(
						&event,
						&connection.event_types,
						&connection.filter,
					);

					if should_forward {
						// Ignore errors if connection is closed
						let _ = connection
							.response_tx
							.send(DaemonResponse::Event(event.clone()));
					}
				}
			}
		});

		// Spawn periodic cleanup task for event buffer
		let buffer_clone = self.event_buffer.clone();
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
			loop {
				interval.tick().await;
				buffer_clone.cleanup_expired().await;
			}
		});

		// Note: Log messages are NOT broadcast as events anymore
		// They use a separate dedicated LogBus (core.logs)
		// Clients subscribe to logs separately, not through the event bus

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
				let include_descendants = filter.include_descendants.unwrap_or(false);
				let affects = event.affects_path(path_scope, include_descendants);

				if !affects {
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
				_ => {}
			}
		}

		true
	}

	/// Handle individual client connection concurrently
	async fn handle_connection(
		stream: tokio::net::TcpStream,
		core: Arc<Core>,
		shutdown_tx: mpsc::Sender<()>,
		connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
		connection_count: Arc<AtomicUsize>,
		event_buffer: Arc<EventBuffer>,
	) -> Result<(), String> {
		let connection_id = Uuid::new_v4();
		let (mut reader, mut writer) = stream.into_split();
		let mut buf_reader = BufReader::new(reader);
		let mut line = String::new();

		// Channel for sending events/logs to this connection
		let (response_tx, mut response_rx) = mpsc::unbounded_channel::<DaemonResponse>();

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
										&response_tx,
										&event_buffer
									).await;

									// Send response
									let response_json = serde_json::to_string(&response)
										.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;

									if let Err(_) = writer.write_all((response_json + "\n").as_bytes()).await {
										break; // Connection closed
									}
									// Flush to ensure response is sent immediately
									if let Err(_) = writer.flush().await {
										break; // Connection closed
									}

									// For non-streaming requests, close connection after response
									match response {
										DaemonResponse::Subscribed | DaemonResponse::LogsSubscribed => {
											// Keep connection open for streaming
										}
										DaemonResponse::Unsubscribed | DaemonResponse::LogsUnsubscribed => {
											// Close connection after unsubscribe
											break;
										}
										DaemonResponse::Event(_) | DaemonResponse::LogMessage(_) => {
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

				// Handle outgoing responses (events/logs) to client
				Some(response) = response_rx.recv() => {
					let response_json = serde_json::to_string(&response)
						.map_err(|e| DaemonError::SerializationError(e.to_string()).to_string())?;

					if let Err(_) = writer.write_all((response_json + "\n").as_bytes()).await {
						break; // Connection closed
					}
					if let Err(_) = writer.flush().await {
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
		response_tx: &mpsc::UnboundedSender<DaemonResponse>,
		event_buffer: &Arc<EventBuffer>,
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
				// Step 1: Get buffered events BEFORE registering connection
				// This prevents race between replay and live events
				let matching_events = event_buffer
					.get_matching_events(&event_types, &filter)
					.await;

				// Step 2: Register connection for event streaming (starts receiving live events)
				let connection = Connection {
					id: connection_id,
					response_tx: response_tx.clone(),
					event_types: event_types.clone(),
					filter: filter.clone(),
					log_filter: None,
				};

				connections.write().await.insert(connection_id, connection);

				// Step 3: Send buffered events in chronological order
				// This ensures no gaps between buffered and live events
				for event in matching_events {
					let _ = response_tx.send(DaemonResponse::Event((*event).clone()));
				}

				DaemonResponse::Subscribed
			}

			DaemonRequest::Unsubscribe => {
				// Remove connection from event streaming
				connections.write().await.remove(&connection_id);
				DaemonResponse::Unsubscribed
			}

			DaemonRequest::SubscribeLogs { filter } => {
				// Start log streaming for this connection
				let mut log_subscriber = core.logs.subscribe();
				let tx = response_tx.clone();
				let filter_clone = filter.clone();

				// Spawn task to forward log messages
				tokio::spawn(async move {
					while let Ok(log_msg) = log_subscriber.recv().await {
						// Apply filter if specified
						if let Some(ref f) = filter_clone {
							// Filter by job_id
							if let Some(ref filter_job_id) = f.job_id {
								if log_msg.job_id.as_ref() != Some(filter_job_id) {
									continue;
								}
							}

							// Filter by library_id
							if let Some(ref filter_library_id) = f.library_id {
								if log_msg.library_id.as_ref() != Some(filter_library_id) {
									continue;
								}
							}

							// Filter by level
							if let Some(ref filter_level) = f.level {
								if !log_msg.level.eq_ignore_ascii_case(filter_level) {
									continue;
								}
							}

							// Filter by target
							if let Some(ref filter_target) = f.target {
								if !log_msg.target.contains(filter_target) {
									continue;
								}
							}
						}

						// Send log message to client
						if tx.send(DaemonResponse::LogMessage(log_msg)).is_err() {
							break; // Connection closed
						}
					}
				});

				DaemonResponse::LogsSubscribed
			}

			DaemonRequest::UnsubscribeLogs => {
				// Log subscription cleanup happens automatically when connection closes
				DaemonResponse::LogsUnsubscribed
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
