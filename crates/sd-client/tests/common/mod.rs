//! In-process mock daemon speaking the real NDJSON protocol.
//!
//! Mirrors the daemon's connection semantics so broker tests exercise the
//! behavior the broker exists to work around:
//! - non-subscribe requests get exactly one response, then the connection
//!   closes;
//! - `Subscribe` keeps the connection open, and a repeat `Subscribe` on the
//!   same connection replaces that connection's subscription;
//! - configured replay events are sent right after each `Subscribed` ack,
//!   like the daemon's event buffer.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use sd_client::{DaemonRequest, DaemonResponse, Event, EventFilter};

/// One `Subscribe` request as the mock saw it.
#[derive(Clone)]
pub struct SubscribeRecord {
	pub connection: u64,
	pub event_types: Vec<String>,
	pub filter: Option<EventFilter>,
}

enum Frame {
	Response(Box<DaemonResponse>),
	Close,
}

struct ConnState {
	frames: mpsc::UnboundedSender<Frame>,
	event_types: Vec<String>,
	filter: Option<EventFilter>,
}

#[derive(Default)]
struct MockState {
	next_conn: u64,
	subscribes: Vec<SubscribeRecord>,
	conns: HashMap<u64, ConnState>,
	replay: Vec<Event>,
	refuse: bool,
	accepts: Vec<Instant>,
}

pub struct MockDaemon {
	addr: SocketAddr,
	state: Arc<Mutex<MockState>>,
	accept_handle: tokio::task::JoinHandle<()>,
}

impl Drop for MockDaemon {
	fn drop(&mut self) {
		self.accept_handle.abort();
	}
}

impl MockDaemon {
	pub async fn start() -> Self {
		let listener = TcpListener::bind("127.0.0.1:0")
			.await
			.expect("bind mock daemon");
		let addr = listener.local_addr().expect("mock daemon addr");
		let state = Arc::new(Mutex::new(MockState::default()));

		let accept_state = state.clone();
		let accept_handle = tokio::spawn(async move {
			loop {
				let Ok((stream, _)) = listener.accept().await else {
					break;
				};
				let refuse = {
					let mut s = accept_state.lock().unwrap();
					s.accepts.push(Instant::now());
					s.refuse
				};
				if refuse {
					// Accept then close without responding, so clients see a
					// failed subscription attempt.
					drop(stream);
					continue;
				}
				tokio::spawn(handle_conn(stream, accept_state.clone()));
			}
		});

		Self {
			addr,
			state,
			accept_handle,
		}
	}

	pub fn addr(&self) -> String {
		self.addr.to_string()
	}

	fn state(&self) -> MutexGuard<'_, MockState> {
		self.state.lock().unwrap()
	}

	/// Every `Subscribe` request received, in arrival order.
	pub fn subscribes(&self) -> Vec<SubscribeRecord> {
		self.state().subscribes.clone()
	}

	pub fn subscribe_count(&self) -> usize {
		self.state().subscribes.len()
	}

	/// Connections currently holding an active subscription.
	pub fn open_subscription_count(&self) -> usize {
		self.state().conns.len()
	}

	/// When enabled, accepted connections are closed without a response.
	pub fn set_refuse(&self, refuse: bool) {
		self.state().refuse = refuse;
	}

	/// Events sent right after each subsequent `Subscribed` ack.
	pub fn set_replay(&self, events: Vec<Event>) {
		self.state().replay = events;
	}

	/// Timestamps of every accepted connection.
	pub fn accept_times(&self) -> Vec<Instant> {
		self.state().accepts.clone()
	}

	/// Deliver an event to every subscription it matches.
	pub fn emit(&self, event: Event) {
		let state = self.state();
		for conn in state.conns.values() {
			if matches(&event, &conn.event_types, &conn.filter) {
				let _ = conn
					.frames
					.send(Frame::Response(Box::new(DaemonResponse::Event(
						event.clone(),
					))));
			}
		}
	}

	/// Close every open subscription connection from the daemon side.
	pub fn kill_subscriptions(&self) {
		for conn in self.state().conns.values() {
			let _ = conn.frames.send(Frame::Close);
		}
	}
}

/// Subset of the daemon's event matching sufficient for these tests:
/// event-type inclusion list plus the library-id filter on library events.
fn matches(event: &Event, event_types: &[String], filter: &Option<EventFilter>) -> bool {
	if !event_types.is_empty() && !event_types.iter().any(|t| t == event.variant_name()) {
		return false;
	}
	if let Some(filter) = filter {
		if let Some(library_id) = filter.library_id {
			match event {
				Event::LibraryCreated { id, .. }
				| Event::LibraryOpened { id, .. }
				| Event::LibraryClosed { id, .. }
					if *id != library_id =>
				{
					return false;
				}
				_ => {}
			}
		}
	}
	true
}

async fn write_response(
	writer: &mut tokio::net::tcp::OwnedWriteHalf,
	response: &DaemonResponse,
) -> std::io::Result<()> {
	let mut json = serde_json::to_string(response).expect("response serializes");
	json.push('\n');
	writer.write_all(json.as_bytes()).await?;
	writer.flush().await
}

async fn handle_conn(stream: TcpStream, state: Arc<Mutex<MockState>>) {
	let conn_id = {
		let mut s = state.lock().unwrap();
		s.next_conn += 1;
		s.next_conn
	};
	let (reader, mut writer) = stream.into_split();
	let mut lines = BufReader::new(reader).lines();
	let (frame_tx, mut frame_rx) = mpsc::unbounded_channel::<Frame>();

	loop {
		tokio::select! {
			line = lines.next_line() => {
				let Ok(Some(line)) = line else { break };
				let Ok(request) = serde_json::from_str::<DaemonRequest>(line.trim()) else {
					break;
				};
				match request {
					DaemonRequest::Ping => {
						let _ = write_response(&mut writer, &DaemonResponse::Pong).await;
						break;
					}
					DaemonRequest::Query { method, library_id, .. }
					| DaemonRequest::Action { method, library_id, .. } => {
						// Echo the envelope so tests can assert what a client
						// injected; one response, then close.
						let response = DaemonResponse::JsonOk(serde_json::json!({
							"method": method,
							"library_id": library_id,
						}));
						let _ = write_response(&mut writer, &response).await;
						break;
					}
					DaemonRequest::Subscribe { event_types, filter } => {
						let replay = {
							let mut s = state.lock().unwrap();
							s.subscribes.push(SubscribeRecord {
								connection: conn_id,
								event_types: event_types.clone(),
								filter: filter.clone(),
							});
							// A repeat Subscribe on this connection replaces
							// the connection's previous subscription.
							s.conns.insert(
								conn_id,
								ConnState {
									frames: frame_tx.clone(),
									event_types,
									filter,
								},
							);
							s.replay.clone()
						};
						if write_response(&mut writer, &DaemonResponse::Subscribed)
							.await
							.is_err()
						{
							break;
						}
						let mut failed = false;
						for event in replay {
							if write_response(&mut writer, &DaemonResponse::Event(event))
								.await
								.is_err()
							{
								failed = true;
								break;
							}
						}
						if failed {
							break;
						}
					}
					DaemonRequest::Unsubscribe => {
						state.lock().unwrap().conns.remove(&conn_id);
						let _ = write_response(&mut writer, &DaemonResponse::Unsubscribed).await;
						break;
					}
					_ => break,
				}
			}
			frame = frame_rx.recv() => match frame {
				Some(Frame::Response(response)) => {
					if write_response(&mut writer, response.as_ref()).await.is_err() {
						break;
					}
				}
				Some(Frame::Close) | None => break,
			}
		}
	}

	state.lock().unwrap().conns.remove(&conn_id);
}

/// Poll `cond` until it holds or `timeout` elapses (then panic).
pub async fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) {
	let deadline = Instant::now() + timeout;
	while !cond() {
		assert!(
			Instant::now() < deadline,
			"condition not met within {timeout:?}"
		);
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
}
