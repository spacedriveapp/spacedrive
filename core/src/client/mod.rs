use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::marker::PhantomData;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::infra::daemon::client::DaemonClient;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse, EventFilter, LogFilter};
use crate::infra::event::Event;
use crate::infra::event::log_emitter::LogMessage;

pub trait Wire {
	const METHOD: &'static str;
}

#[derive(Clone)]
pub struct CoreClient {
	daemon: DaemonClient,
}

impl CoreClient {
	pub fn new(socket: PathBuf) -> Self {
		Self {
			daemon: DaemonClient::new(socket),
		}
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
		let payload = serde_json::to_value(query)?;
		let resp = self
			.daemon
			.send(&DaemonRequest::Query {
				method: Q::METHOD.into(),
				library_id,
				payload,
			})
			.await;
		match resp {
			Ok(r) => match r {
				DaemonResponse::JsonOk(json) => {
					let result = serde_json::from_value(json)?;
					Ok(result)
				}
				DaemonResponse::Error(e) => Err(anyhow::anyhow!(e.to_string())),
				other => Err(anyhow::anyhow!(format!("unexpected response: {:?}", other))),
			},
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
		LogStream::new(self.daemon.clone(), filter).await
	}
}

/// Stream of events from the core
pub struct EventStream {
	daemon: DaemonClient,
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
		let daemon_clone = daemon.clone();
		let handle = tokio::spawn(async move {
			if let Err(e) = Self::stream_events(daemon_clone, event_types, filter, event_tx).await {
				eprintln!("Event streaming error: {}", e);
			}
		});

		Ok(Self {
			daemon,
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
	daemon: DaemonClient,
	log_rx: mpsc::UnboundedReceiver<LogMessage>,
	_handle: tokio::task::JoinHandle<()>,
}

impl LogStream {
	async fn new(daemon: DaemonClient, filter: Option<LogFilter>) -> Result<Self> {
		let (log_tx, log_rx) = mpsc::unbounded_channel();

		// Start streaming connection
		let daemon_clone = daemon.clone();
		let handle = tokio::spawn(async move {
			if let Err(e) = Self::stream_logs(daemon_clone, filter, log_tx).await {
				eprintln!("Log streaming error: {}", e);
			}
		});

		Ok(Self {
			daemon,
			log_rx,
			_handle: handle,
		})
	}

	async fn stream_logs(
		daemon: DaemonClient,
		filter: Option<LogFilter>,
		log_tx: mpsc::UnboundedSender<LogMessage>,
	) -> Result<()> {
		let request = DaemonRequest::SubscribeLogs { filter };

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
