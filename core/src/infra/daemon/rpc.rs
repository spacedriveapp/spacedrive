use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;

use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::infra::daemon::instance::CoreInstanceManager;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse, EventFilter};
use crate::infra::event::{Event, EventSubscriber};

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
	instances: Arc<CoreInstanceManager>,
	shutdown_tx: mpsc::Sender<()>,
	shutdown_rx: mpsc::Receiver<()>,
	/// Active connections for event streaming
	connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
}

impl RpcServer {
	pub fn new(socket_path: PathBuf, instances: Arc<CoreInstanceManager>) -> Self {
		let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
		Self {
			socket_path,
			instances,
			shutdown_tx,
			shutdown_rx,
			connections: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
		if std::fs::remove_file(&self.socket_path).is_ok() {}
		if let Some(parent) = self.socket_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let listener = UnixListener::bind(&self.socket_path)?;

		// Start event broadcaster
		self.start_event_broadcaster().await?;

		loop {
			tokio::select! {
				// Handle new connections
				result = listener.accept() => {
					match result {
						Ok((stream, _addr)) => {
							let instances = self.instances.clone();
							let shutdown_tx = self.shutdown_tx.clone();
							let connections = self.connections.clone();

							// Spawn task for concurrent request handling
							tokio::spawn(async move {
								// Convert errors to strings to ensure Send
								if let Err(e) = Self::handle_connection(stream, instances, shutdown_tx, connections).await {
									eprintln!("Connection error: {}", e);
								}
							});
						}
						Err(e) => {
							eprintln!("Accept error: {}", e);
							continue;
						}
					}
				}

				// Handle shutdown signal
				_ = self.shutdown_rx.recv() => {
					eprintln!("Shutdown signal received, stopping RPC server");
					break;
				}
			}
		}

		Ok(())
	}

	/// Start the event broadcaster that forwards core events to subscribed connections
	async fn start_event_broadcaster(&self) -> Result<(), Box<dyn std::error::Error>> {
		let core = self.instances.get_default().await?;
		let mut event_subscriber = core.events.subscribe();
		let connections = self.connections.clone();

		tokio::spawn(async move {
			while let Ok(event) = event_subscriber.recv().await {
				let connections_read = connections.read().await;

				// Broadcast event to all subscribed connections
				for connection in connections_read.values() {
					if Self::should_forward_event(&event, &connection.event_types, &connection.filter) {
						// Ignore errors if connection is closed
						let _ = connection.event_tx.send(event.clone());
					}
				}
			}
		});

		Ok(())
	}

	/// Check if an event should be forwarded to a connection based on filters
	fn should_forward_event(event: &Event, event_types: &[String], filter: &Option<EventFilter>) -> bool {
		// If no specific event types requested, forward all events
		if event_types.is_empty() {
			return true;
		}

		// Check if event type matches subscription
		let event_type = match event {
			Event::JobProgress { .. } => "JobProgress",
			Event::JobStarted { .. } => "JobStarted",
			Event::JobCompleted { .. } => "JobCompleted",
			Event::JobFailed { .. } => "JobFailed",
			Event::JobCancelled { .. } => "JobCancelled",
			Event::JobPaused { .. } => "JobPaused",
			Event::JobResumed { .. } => "JobResumed",
			Event::LibraryCreated { .. } => "LibraryCreated",
			Event::LibraryOpened { .. } => "LibraryOpened",
			Event::LibraryClosed { .. } => "LibraryClosed",
			Event::IndexingStarted { .. } => "IndexingStarted",
			Event::IndexingProgress { .. } => "IndexingProgress",
			Event::IndexingCompleted { .. } => "IndexingCompleted",
			Event::LogMessage { .. } => "LogMessage",
			_ => "Other",
		};

		if !event_types.contains(&event_type.to_string()) {
			return false;
		}

		// Apply additional filters if specified
		if let Some(filter) = filter {
			match event {
				Event::JobProgress { job_id, .. } |
				Event::JobStarted { job_id, .. } |
				Event::JobCompleted { job_id, .. } |
				Event::JobFailed { job_id, .. } |
				Event::JobCancelled { job_id, .. } => {
					if let Some(filter_job_id) = &filter.job_id {
						return job_id == filter_job_id;
					}
				}
				Event::LibraryCreated { id, .. } |
				Event::LibraryOpened { id, .. } |
				Event::LibraryClosed { id, .. } => {
					if let Some(filter_library_id) = &filter.library_id {
						return id == filter_library_id;
					}
				}
				Event::LogMessage { job_id, library_id, .. } => {
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
		instances: Arc<CoreInstanceManager>,
		shutdown_tx: mpsc::Sender<()>,
		connections: Arc<RwLock<HashMap<Uuid, Connection>>>,
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
							if let Ok(request) = serde_json::from_str::<DaemonRequest>(&line.trim()) {
								let response = Self::process_request(
									request,
									&instances,
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
		Ok(())
	}

	/// Process a parsed daemon request
	async fn process_request(
		request: DaemonRequest,
		instances: &Arc<CoreInstanceManager>,
		shutdown_tx: &mpsc::Sender<()>,
		connections: &Arc<RwLock<HashMap<Uuid, Connection>>>,
		connection_id: Uuid,
		event_tx: &mpsc::UnboundedSender<Event>,
	) -> DaemonResponse {
		match request {
			DaemonRequest::Ping => DaemonResponse::Pong,

			DaemonRequest::Action { method, payload } => match instances.get_default().await {
				Ok(core) => match core.execute_operation_by_method(&method, payload).await {
					Ok(out) => DaemonResponse::Ok(out),
					Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
				},
				Err(e) => DaemonResponse::Error(DaemonError::CoreUnavailable(e)),
			},

			DaemonRequest::Query { method, payload } => match instances.get_default().await {
				Ok(core) => match core.execute_operation_by_method(&method, payload).await {
					Ok(out) => DaemonResponse::Ok(out),
					Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
				},
				Err(e) => DaemonResponse::Error(DaemonError::CoreUnavailable(e)),
			},

			DaemonRequest::Subscribe { event_types, filter } => {
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
}
