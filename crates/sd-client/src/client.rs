use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

use sd_core::infra::action::preflight::Validation;
use sd_core::infra::daemon::client::DaemonClient;
use sd_core::infra::daemon::types::{DaemonRequest, DaemonResponse, EventFilter, LogFilter};
use sd_core::infra::event::log_emitter::LogMessage;
use sd_core::infra::event::Event;
use sd_core::infra::wire::{preflight_method, Preflight, Wire};

/// The wire method of a preflight check over an action's input type.
fn preflight(action_method: &str, kind: Preflight) -> Result<String> {
	preflight_method(action_method, kind)
		.ok_or_else(|| anyhow::anyhow!("{action_method} is not an action"))
}

#[derive(Clone)]
pub struct CoreClient {
	daemon: DaemonClient,
	/// A paired device every action and query is targeted at, by name,
	/// slug, or id. The local daemon forwards the operation there.
	device: Option<String>,
}

impl CoreClient {
	pub fn new(socket_addr: String) -> Self {
		Self {
			daemon: DaemonClient::new(socket_addr),
			device: None,
		}
	}

	pub fn with_device(mut self, device: Option<String>) -> Self {
		self.device = device;
		self
	}

	pub fn device(&self) -> Option<&str> {
		self.device.as_deref()
	}

	pub async fn action<A>(
		&self,
		action: &A,
		library_id: Option<uuid::Uuid>,
	) -> Result<serde_json::Value>
	where
		A: Wire + Serialize,
	{
		let payload = serde_json::to_value(action)?;
		let resp = self
			.daemon
			.send(&DaemonRequest::Action {
				method: A::METHOD.into(),
				library_id,
				payload,
				device: self.device.clone(),
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::JsonOk(json) => Ok(json),
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e.to_string())),
				other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			},
			Err(e) => Err(anyhow::anyhow!(e.to_string())),
		}
	}

	pub async fn query<Q, O>(&self, query: &Q, library_id: Option<uuid::Uuid>) -> Result<O>
	where
		Q: Wire + Serialize,
		O: DeserializeOwned,
	{
		let json = self
			.read(Q::METHOD.into(), serde_json::to_value(query)?, library_id)
			.await?;
		Ok(serde_json::from_value(json)?)
	}

	/// Whether and how an action would run, over the input it takes.
	pub async fn validate<A>(
		&self,
		action: &A,
		library_id: Option<uuid::Uuid>,
	) -> Result<Validation>
	where
		A: Wire + Serialize,
	{
		let json = self
			.read(
				preflight(A::METHOD, Preflight::Validate)?,
				serde_json::to_value(action)?,
				library_id,
			)
			.await?;
		Ok(serde_json::from_value(json)?)
	}

	/// What would exist after an action, over the input it takes: its plan.
	pub async fn preview<A, P>(&self, action: &A, library_id: Option<uuid::Uuid>) -> Result<P>
	where
		A: Wire + Serialize,
		P: DeserializeOwned,
	{
		let json = self
			.read(
				preflight(A::METHOD, Preflight::Preview)?,
				serde_json::to_value(action)?,
				library_id,
			)
			.await?;
		Ok(serde_json::from_value(json)?)
	}

	/// One read, a query or a preflight method, answered as JSON.
	async fn read(
		&self,
		method: String,
		payload: serde_json::Value,
		library_id: Option<uuid::Uuid>,
	) -> Result<serde_json::Value> {
		let resp = self
			.daemon
			.send(&DaemonRequest::Query {
				method,
				library_id,
				payload,
				device: self.device.clone(),
			})
			.await;
		match resp {
			Ok(DaemonResponse::JsonOk(json)) => Ok(json),
			Ok(DaemonResponse::Error(e)) => Err(anyhow::anyhow!(e.to_string())),
			Ok(other) => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			Err(e) => Err(anyhow::anyhow!(e.to_string())),
		}
	}

	pub async fn send_raw_request(&self, req: &DaemonRequest) -> Result<DaemonResponse> {
		self.daemon
			.send(req)
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))
	}

	/// Subscribe to real-time events from the core
	pub async fn subscribe_events(
		&self,
		event_types: Vec<String>,
		filter: Option<EventFilter>,
	) -> Result<EventStream> {
		EventStream::new(self.daemon.clone(), event_types, filter).await
	}

	/// Subscribe to real-time log messages from the core
	pub async fn subscribe_logs(
		&self,
		job_id: Option<String>,
		level: Option<String>,
		target: Option<String>,
	) -> Result<LogStream> {
		let filter = if job_id.is_some() || level.is_some() || target.is_some() {
			Some(LogFilter {
				library_id: None,
				job_id,
				level,
				target,
			})
		} else {
			None
		};
		LogStream::new(self.daemon.clone(), filter, self.device.clone()).await
	}
}

/// Stream of events from the core
pub struct EventStream {
	event_rx: mpsc::UnboundedReceiver<Event>,
	_handle: tokio::task::JoinHandle<()>,
}

impl EventStream {
	async fn new(
		daemon: DaemonClient,
		event_types: Vec<String>,
		filter: Option<EventFilter>,
	) -> Result<Self> {
		let (event_tx, event_rx) = mpsc::unbounded_channel();

		// Start streaming connection
		let handle = tokio::spawn(async move {
			if let Err(e) = Self::stream_events(daemon, event_types, filter, event_tx).await {
				eprintln!("Event streaming error: {}", e);
			}
		});

		Ok(Self {
			event_rx,
			_handle: handle,
		})
	}

	async fn stream_events(
		daemon: DaemonClient,
		event_types: Vec<String>,
		filter: Option<EventFilter>,
		event_tx: mpsc::UnboundedSender<Event>,
	) -> Result<()> {
		let request = DaemonRequest::Subscribe {
			event_types,
			filter,
		};

		// Stream events
		daemon
			.stream(&request, event_tx)
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))?;

		Ok(())
	}

	/// Receive the next event
	pub async fn recv(&mut self) -> Option<Event> {
		self.event_rx.recv().await
	}

	/// Try to receive an event without blocking
	pub fn try_recv(&mut self) -> Result<Event, mpsc::error::TryRecvError> {
		self.event_rx.try_recv()
	}
}

/// Stream of log messages from the core
pub struct LogStream {
	log_rx: mpsc::UnboundedReceiver<LogMessage>,
	_handle: tokio::task::JoinHandle<()>,
}

impl LogStream {
	async fn new(
		daemon: DaemonClient,
		filter: Option<LogFilter>,
		device: Option<String>,
	) -> Result<Self> {
		let (log_tx, log_rx) = mpsc::unbounded_channel();

		// Start streaming connection
		let handle = tokio::spawn(async move {
			if let Err(e) = Self::stream_logs(daemon, filter, device, log_tx).await {
				eprintln!("Log streaming error: {}", e);
			}
		});

		Ok(Self {
			log_rx,
			_handle: handle,
		})
	}

	async fn stream_logs(
		daemon: DaemonClient,
		filter: Option<LogFilter>,
		device: Option<String>,
		log_tx: mpsc::UnboundedSender<LogMessage>,
	) -> Result<()> {
		let request = DaemonRequest::SubscribeLogs { filter, device };

		// Use the same stream infrastructure but for log messages
		daemon
			.stream_logs(&request, log_tx)
			.await
			.map_err(|e| anyhow::anyhow!(e.to_string()))?;

		Ok(())
	}

	/// Receive the next log message
	pub async fn recv(&mut self) -> Option<LogMessage> {
		self.log_rx.recv().await
	}

	/// Try to receive a log message without blocking
	pub fn try_recv(&mut self) -> Result<LogMessage, mpsc::error::TryRecvError> {
		self.log_rx.try_recv()
	}
}
