use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;
use std::marker::PhantomData;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::infra::daemon::client::DaemonClient;
use crate::infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse, EventFilter};
use crate::infra::event::Event;

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

		// This would need to be implemented in DaemonClient to support streaming
		// For now, we'll use a placeholder that shows the concept
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
