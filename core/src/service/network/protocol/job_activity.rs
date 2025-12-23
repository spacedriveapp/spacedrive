//! Job activity protocol for sharing job status across devices

use super::{ProtocolEvent, ProtocolHandler};
use crate::{
	infra::{
		event::{Event, EventBus},
		job::{generic_progress::GenericProgress, output::JobOutput, types::JobStatus},
	},
	service::network::{
		device::DeviceRegistry,
		utils::{self, get_or_create_connection},
		NetworkingError, Result,
	},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use iroh::{endpoint::Connection, Endpoint, NodeId};
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	sync::{broadcast, Mutex, RwLock},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub use super::super::core::JOB_ACTIVITY_ALPN;

/// Messages exchanged in the job activity protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobActivityMessage {
	/// Subscribe to job events from this device
	Subscribe {
		/// Optional: filter by library_id
		library_id: Option<Uuid>,
	},

	/// Unsubscribe from job events
	Unsubscribe,

	/// Job event notification (one-way broadcast)
	JobEvent {
		/// ID of the library this job belongs to
		library_id: Uuid,

		/// Device ID that's running the job
		device_id: Uuid,

		/// Event payload
		event: RemoteJobEvent,
	},
}

/// Remote job events that can be broadcast to other devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteJobEvent {
	JobQueued {
		job_id: String,
		job_type: String,
		timestamp: DateTime<Utc>,
	},
	JobStarted {
		job_id: String,
		job_type: String,
		timestamp: DateTime<Utc>,
	},
	JobProgress {
		job_id: String,
		job_type: String,
		progress: f64,
		message: Option<String>,
		generic_progress: Option<GenericProgress>,
		timestamp: DateTime<Utc>,
	},
	JobCompleted {
		job_id: String,
		job_type: String,
		output: JobOutput,
		timestamp: DateTime<Utc>,
	},
	JobFailed {
		job_id: String,
		job_type: String,
		error: String,
		timestamp: DateTime<Utc>,
	},
	JobCancelled {
		job_id: String,
		job_type: String,
		timestamp: DateTime<Utc>,
	},
	JobPaused {
		job_id: String,
		timestamp: DateTime<Utc>,
	},
	JobResumed {
		job_id: String,
		timestamp: DateTime<Utc>,
	},
}

/// Subscription information for a remote device
struct Subscription {
	node_id: NodeId,
	event_tx: tokio::sync::mpsc::UnboundedSender<JobActivityMessage>,
	library_filter: Option<Uuid>,
	last_activity: DateTime<Utc>,
}

/// Progress throttle to limit network traffic
struct ProgressThrottle {
	last_sent: HashMap<String, Instant>,
	min_interval: Duration,
}

impl ProgressThrottle {
	fn new(min_interval: Duration) -> Self {
		Self {
			last_sent: HashMap::new(),
			min_interval,
		}
	}

	fn should_send(&mut self, job_id: &str) -> bool {
		let now = Instant::now();

		if let Some(last) = self.last_sent.get(job_id) {
			if now.duration_since(*last) < self.min_interval {
				return false;
			}
		}

		self.last_sent.insert(job_id.to_string(), now);
		true
	}

	fn cleanup(&mut self, job_id: &str) {
		self.last_sent.remove(job_id);
	}
}

/// Protocol handler for job activity sharing
pub struct JobActivityProtocolHandler {
	/// Event bus for subscribing to job events
	event_bus: Arc<EventBus>,

	/// Device registry for node_id → device_id mapping
	device_registry: Arc<RwLock<DeviceRegistry>>,

	/// Endpoint for creating connections
	endpoint: Option<Endpoint>,

	/// Active subscriptions: device_id → subscription info
	subscriptions: Arc<RwLock<HashMap<Uuid, Subscription>>>,

	/// Cached connections (shared with NetworkingService)
	connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,

	/// Local device ID
	device_id: Uuid,

	/// Library ID for filtering (optional)
	library_id: Option<Uuid>,

	/// Progress throttle for network efficiency
	throttle: Arc<Mutex<ProgressThrottle>>,
}

impl JobActivityProtocolHandler {
	/// Create a new job activity protocol handler
	pub fn new(
		event_bus: Arc<EventBus>,
		device_registry: Arc<RwLock<DeviceRegistry>>,
		endpoint: Option<Endpoint>,
		connections: Arc<RwLock<HashMap<(NodeId, Vec<u8>), Connection>>>,
		device_id: Uuid,
		library_id: Option<Uuid>,
	) -> Self {
		let handler = Self {
			event_bus,
			device_registry,
			endpoint,
			subscriptions: Arc::new(RwLock::new(HashMap::new())),
			connections,
			device_id,
			library_id,
			throttle: Arc::new(Mutex::new(ProgressThrottle::new(Duration::from_millis(
				500,
			)))),
		};

		// Start listening to event bus for job events
		handler.start_event_listener();

		handler
	}

	/// Start listening to the event bus and broadcasting job events
	fn start_event_listener(&self) {
		let event_bus = self.event_bus.clone();
		let subscriptions = self.subscriptions.clone();
		let device_id = self.device_id;
		let library_id = self.library_id;
		let throttle = self.throttle.clone();

		tokio::spawn(async move {
			let mut subscriber = event_bus.subscribe();

			loop {
				match subscriber.recv().await {
					Ok(event) => {
						// Only broadcast job events
						if !Self::is_job_event(&event) {
							continue;
						}

						// Convert to remote job event
						let remote_event =
							match Self::convert_event(&event, &mut *throttle.lock().await) {
								Some(e) => e,
								None => continue,
							};

						// Broadcast to all subscribed devices
						let message = JobActivityMessage::JobEvent {
							library_id: library_id.unwrap_or_default(),
							device_id,
							event: remote_event.clone(),
						};

						Self::broadcast_to_subscribers(subscriptions.clone(), library_id, message)
							.await;

						// Cleanup throttle for completed/failed/cancelled jobs
						if matches!(
							remote_event,
							RemoteJobEvent::JobCompleted { .. }
								| RemoteJobEvent::JobFailed { .. }
								| RemoteJobEvent::JobCancelled { .. }
						) {
							if let RemoteJobEvent::JobCompleted { job_id, .. }
							| RemoteJobEvent::JobFailed { job_id, .. }
							| RemoteJobEvent::JobCancelled { job_id, .. } = remote_event
							{
								throttle.lock().await.cleanup(&job_id);
							}
						}
					}
					Err(e) => {
						error!("Job activity event listener error: {}", e);
						break;
					}
				}
			}
		});
	}

	/// Check if an event is a job event
	fn is_job_event(event: &Event) -> bool {
		matches!(
			event,
			Event::JobQueued { .. }
				| Event::JobStarted { .. }
				| Event::JobProgress { .. }
				| Event::JobCompleted { .. }
				| Event::JobFailed { .. }
				| Event::JobCancelled { .. }
				| Event::JobPaused { .. }
				| Event::JobResumed { .. }
		)
	}

	/// Convert a local Event to a RemoteJobEvent with throttling
	fn convert_event(event: &Event, throttle: &mut ProgressThrottle) -> Option<RemoteJobEvent> {
		match event {
			Event::JobQueued {
				job_id, job_type, ..
			} => Some(RemoteJobEvent::JobQueued {
				job_id: job_id.clone(),
				job_type: job_type.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobStarted {
				job_id, job_type, ..
			} => Some(RemoteJobEvent::JobStarted {
				job_id: job_id.clone(),
				job_type: job_type.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobProgress {
				job_id,
				job_type,
				progress,
				message,
				generic_progress,
				..
			} => {
				// Apply throttling for progress events
				if !throttle.should_send(job_id) {
					return None;
				}

				Some(RemoteJobEvent::JobProgress {
					job_id: job_id.clone(),
					job_type: job_type.clone(),
					progress: *progress,
					message: message.clone(),
					generic_progress: generic_progress.clone(),
					timestamp: Utc::now(),
				})
			}
			Event::JobCompleted {
				job_id,
				job_type,
				output,
				..
			} => Some(RemoteJobEvent::JobCompleted {
				job_id: job_id.clone(),
				job_type: job_type.clone(),
				output: output.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobFailed {
				job_id,
				job_type,
				error,
				..
			} => Some(RemoteJobEvent::JobFailed {
				job_id: job_id.clone(),
				job_type: job_type.clone(),
				error: error.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobCancelled {
				job_id, job_type, ..
			} => Some(RemoteJobEvent::JobCancelled {
				job_id: job_id.clone(),
				job_type: job_type.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobPaused { job_id, .. } => Some(RemoteJobEvent::JobPaused {
				job_id: job_id.clone(),
				timestamp: Utc::now(),
			}),
			Event::JobResumed { job_id, .. } => Some(RemoteJobEvent::JobResumed {
				job_id: job_id.clone(),
				timestamp: Utc::now(),
			}),
			_ => None,
		}
	}

	/// Broadcast a message to all subscribed devices
	async fn broadcast_to_subscribers(
		subscriptions: Arc<RwLock<HashMap<Uuid, Subscription>>>,
		library_filter: Option<Uuid>,
		message: JobActivityMessage,
	) {
		let subs = subscriptions.read().await;

		for (device_id, subscription) in subs.iter() {
			// Apply library filter if subscription has one
			if let (Some(sub_lib), JobActivityMessage::JobEvent { library_id, .. }) =
				(subscription.library_filter, &message)
			{
				if sub_lib != *library_id {
					continue;
				}
			}

			// Send to the channel (will be sent over stream by handle_stream)
			if subscription.event_tx.send(message.clone()).is_err() {
				debug!("Failed to send to device {} (channel closed)", device_id);
			}
		}
	}

	/// Handle device disconnection
	pub async fn handle_device_disconnect(&self, device_id: Uuid) {
		let mut subs = self.subscriptions.write().await;
		if subs.remove(&device_id).is_some() {
			info!("Removed subscription for device {}", device_id);
		}
	}
}

#[async_trait]
impl ProtocolHandler for JobActivityProtocolHandler {
	fn protocol_name(&self) -> &str {
		"job_activity"
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	async fn handle_stream(
		&self,
		mut send: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
		mut recv: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
		remote_node_id: NodeId,
	) {
		// Create channel for receiving events to send
		let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

		// Read length-prefixed message to get subscription request
		let mut len_buf = [0u8; 4];
		if recv.read_exact(&mut len_buf).await.is_err() {
			return;
		}
		let msg_len = u32::from_be_bytes(len_buf) as usize;

		let mut msg_buf = vec![0u8; msg_len];
		if recv.read_exact(&mut msg_buf).await.is_err() {
			return;
		}

		// Deserialize subscribe message
		let message: JobActivityMessage = match rmp_serde::from_slice(&msg_buf) {
			Ok(m) => m,
			Err(e) => {
				error!("Failed to deserialize: {}", e);
				return;
			}
		};

		let (device_id, library_filter) = match message {
			JobActivityMessage::Subscribe { library_id } => {
				// Get device_id from node_id
				let device_id = {
					let registry = self.device_registry.read().await;
					match registry.get_device_by_node(remote_node_id) {
						Some(id) => id,
						None => {
							warn!("Unknown device for node {}", remote_node_id);
							return;
						}
					}
				};

				info!(
					"Device {} subscribed (library: {:?})",
					device_id, library_id
				);

				(device_id, library_id)
			}
			_ => {
				error!("Expected Subscribe message");
				return;
			}
		};

		// Store subscription
		let subscription = Subscription {
			node_id: remote_node_id,
			event_tx,
			library_filter,
			last_activity: Utc::now(),
		};

		self.subscriptions
			.write()
			.await
			.insert(device_id, subscription);

		// Loop: receive events from channel and write to stream
		while let Some(message) = event_rx.recv().await {
			// Serialize
			let data = match rmp_serde::to_vec(&message) {
				Ok(d) => d,
				Err(e) => {
					error!("Failed to serialize: {}", e);
					continue;
				}
			};

			// Length-prefixed framing
			let len = (data.len() as u32).to_be_bytes();
			if send.write_all(&len).await.is_err()
				|| send.write_all(&data).await.is_err()
				|| send.flush().await.is_err()
			{
				error!("Failed to send to device {}", device_id);
				break;
			}
		}

		// Clean up subscription on disconnect
		self.subscriptions.write().await.remove(&device_id);
		info!("Device {} unsubscribed (stream closed)", device_id);
	}

	async fn handle_request(&self, _: Uuid, _: Vec<u8>) -> Result<Vec<u8>> {
		Ok(Vec::new())
	}

	async fn handle_response(&self, _: Uuid, _: NodeId, _: Vec<u8>) -> Result<()> {
		Ok(())
	}

	async fn handle_event(&self, _: ProtocolEvent) -> Result<()> {
		Ok(())
	}
}
