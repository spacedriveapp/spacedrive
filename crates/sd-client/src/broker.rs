//! Client-side multiplexing of daemon event subscriptions.
//!
//! The daemon keys subscriptions by TCP connection: a second `Subscribe` on
//! the same connection replaces the first, and only subscribe connections
//! stay open at all. It also caps concurrent connections. A windowed client
//! with many views therefore cannot open one connection per interested
//! component — the fan-out has to happen on this side of the socket.
//!
//! [`SubscriptionBroker`] pools subscriptions by their canonical
//! `(event_types, filter)` signature: one daemon connection per distinct
//! signature, shared by every [`BrokerSubscription`] with the same signature.
//! Connections reconnect with capped exponential backoff and re-issue their
//! `Subscribe` request; the daemon replays recently buffered events on
//! subscribe, which covers the gap but means the same event can be delivered
//! twice across a reconnect. Subscribers must tolerate duplicates — events
//! are invalidation hints, not a transactional log.
//!
//! Fan-out uses a bounded broadcast ring per connection. Delivery to the ring
//! never blocks, so a slow subscriber cannot stall the connection or its
//! peers; a subscriber that falls more than the ring capacity behind loses
//! its oldest unread events (drop-oldest) and resumes at the oldest retained
//! one. When the last subscriber for a signature drops, the connection stays
//! open for a linger period before closing, so a view remount does not churn
//! TCP connections.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, watch};

use sd_core::infra::daemon::types::{DaemonRequest, DaemonResponse, EventFilter};
use sd_core::infra::event::Event;

/// Tuning knobs for [`SubscriptionBroker`].
#[derive(Debug, Clone)]
pub struct BrokerOptions {
	/// How long an idle connection (zero subscribers) stays open before the
	/// broker closes it. Bridges unmount/remount cycles in UIs.
	pub linger: Duration,
	/// First reconnect delay after a connection drops or fails to establish.
	pub initial_backoff: Duration,
	/// Ceiling for the exponential reconnect delay.
	pub max_backoff: Duration,
	/// Capacity of the per-connection event ring. A subscriber more than this
	/// many events behind loses its oldest unread events.
	pub channel_capacity: usize,
}

impl Default for BrokerOptions {
	fn default() -> Self {
		Self {
			linger: Duration::from_secs(3),
			initial_backoff: Duration::from_millis(250),
			max_backoff: Duration::from_secs(10),
			channel_capacity: 256,
		}
	}
}

/// Pools daemon event subscriptions so many in-app receivers share one TCP
/// connection per distinct filter signature.
///
/// Cloning the broker is cheap; clones share the same connection pool.
/// [`SubscriptionBroker::subscribe`] must be called from within a tokio
/// runtime — connection tasks are spawned onto it.
#[derive(Clone)]
pub struct SubscriptionBroker {
	inner: Arc<BrokerInner>,
}

struct BrokerInner {
	socket_addr: String,
	options: BrokerOptions,
	state: Mutex<BrokerState>,
}

struct BrokerState {
	entries: HashMap<String, Entry>,
	/// Sender feeding the linger reaper task; created on first subscribe.
	control_tx: Option<mpsc::UnboundedSender<LingerCheck>>,
}

/// One pooled daemon connection and its subscriber accounting.
struct Entry {
	subscriber_count: usize,
	/// Bumped each time the count returns to zero, so a linger check scheduled
	/// for an earlier idle period cannot close a connection that was reclaimed
	/// and released again in the meantime.
	generation: u64,
	events_tx: broadcast::Sender<Event>,
	/// Dropping this ends the connection task; no explicit signal is sent.
	_shutdown_tx: watch::Sender<bool>,
}

/// Scheduled close check for a connection that went idle.
struct LingerCheck {
	key: String,
	generation: u64,
}

impl SubscriptionBroker {
	/// Create a broker for the daemon at `socket_addr` with default options.
	pub fn new(socket_addr: impl Into<String>) -> Self {
		Self::with_options(socket_addr, BrokerOptions::default())
	}

	/// Create a broker with explicit linger, backoff, and capacity settings.
	pub fn with_options(socket_addr: impl Into<String>, options: BrokerOptions) -> Self {
		Self {
			inner: Arc::new(BrokerInner {
				socket_addr: socket_addr.into(),
				options,
				state: Mutex::new(BrokerState {
					entries: HashMap::new(),
					control_tx: None,
				}),
			}),
		}
	}

	/// Subscribe to daemon events matching `event_types` and `filter`.
	///
	/// The request is canonicalized (event types sorted and deduplicated, the
	/// filter normalized) so equivalent subscriptions share one connection.
	/// An empty `event_types` list subscribes to all events.
	///
	/// The connection is established in the background; events flow into the
	/// returned handle as they arrive, including any recent events the daemon
	/// replays on subscribe. Duplicates are possible across reconnects.
	pub fn subscribe(
		&self,
		event_types: Vec<String>,
		filter: Option<EventFilter>,
	) -> BrokerSubscription {
		let (event_types, filter) = canonicalize(event_types, filter);
		let request = DaemonRequest::Subscribe {
			event_types,
			filter,
		};
		// The canonical wire request doubles as the pool key.
		let key = serde_json::to_string(&request).expect("subscribe request serializes to JSON");

		let mut state = lock(&self.inner.state);
		let control_tx = state.ensure_reaper(&self.inner);

		let rx = match state.entries.get_mut(&key) {
			Some(entry) => {
				entry.subscriber_count += 1;
				entry.events_tx.subscribe()
			}
			None => {
				let (events_tx, events_rx) =
					broadcast::channel(self.inner.options.channel_capacity);
				let (shutdown_tx, shutdown_rx) = watch::channel(false);
				tokio::spawn(run_connection(
					self.inner.socket_addr.clone(),
					format!("{key}\n"),
					events_tx.clone(),
					shutdown_rx,
					self.inner.options.initial_backoff,
					self.inner.options.max_backoff,
				));
				state.entries.insert(
					key.clone(),
					Entry {
						subscriber_count: 1,
						generation: 0,
						events_tx,
						_shutdown_tx: shutdown_tx,
					},
				);
				events_rx
			}
		};

		BrokerSubscription {
			rx,
			_guard: SubscriptionGuard {
				inner: self.inner.clone(),
				key,
				control_tx,
			},
		}
	}

	/// Number of daemon connections currently open, lingering ones included.
	pub fn connection_count(&self) -> usize {
		lock(&self.inner.state).entries.len()
	}
}

impl BrokerState {
	/// Start the linger reaper on first use and return a sender feeding it.
	fn ensure_reaper(&mut self, inner: &Arc<BrokerInner>) -> mpsc::UnboundedSender<LingerCheck> {
		if let Some(tx) = &self.control_tx {
			return tx.clone();
		}
		let (tx, rx) = mpsc::unbounded_channel();
		tokio::spawn(run_reaper(Arc::downgrade(inner), rx, inner.options.linger));
		self.control_tx = Some(tx.clone());
		tx
	}
}

/// Receiver handle for one pooled subscription.
///
/// Dropping the handle releases its share of the pooled connection; the
/// connection closes once its last handle is gone and the linger period
/// elapses without a new subscriber.
pub struct BrokerSubscription {
	rx: broadcast::Receiver<Event>,
	_guard: SubscriptionGuard,
}

impl BrokerSubscription {
	/// Receive the next event, waiting until one arrives.
	///
	/// Skips over any events lost to ring overflow (drop-oldest) and resumes
	/// at the oldest retained one. Returns `None` once the broker is gone and
	/// no buffered events remain.
	pub async fn recv(&mut self) -> Option<Event> {
		loop {
			match self.rx.recv().await {
				Ok(event) => return Some(event),
				Err(broadcast::error::RecvError::Lagged(_)) => continue,
				Err(broadcast::error::RecvError::Closed) => return None,
			}
		}
	}

	/// Receive the next event without waiting.
	///
	/// Returns `None` when no event is ready, or when the broker is gone and
	/// no buffered events remain. Overflow is skipped as in [`Self::recv`].
	pub fn try_recv(&mut self) -> Option<Event> {
		loop {
			match self.rx.try_recv() {
				Ok(event) => return Some(event),
				Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
				Err(_) => return None,
			}
		}
	}
}

/// Reference-counting guard tying a subscription handle to its pool entry.
struct SubscriptionGuard {
	inner: Arc<BrokerInner>,
	key: String,
	control_tx: mpsc::UnboundedSender<LingerCheck>,
}

impl Drop for SubscriptionGuard {
	fn drop(&mut self) {
		let mut state = lock(&self.inner.state);
		if let Some(entry) = state.entries.get_mut(&self.key) {
			entry.subscriber_count -= 1;
			if entry.subscriber_count == 0 {
				entry.generation += 1;
				let _ = self.control_tx.send(LingerCheck {
					key: self.key.clone(),
					generation: entry.generation,
				});
			}
		}
	}
}

/// Normalize a subscription request so equivalent requests produce the same
/// pool key: event types sorted and deduplicated, `include_descendants`
/// pinned to its effective value, and a filter with no criteria collapsed to
/// no filter at all (the daemon treats them identically).
fn canonicalize(
	mut event_types: Vec<String>,
	filter: Option<EventFilter>,
) -> (Vec<String>, Option<EventFilter>) {
	event_types.sort();
	event_types.dedup();
	let filter = filter.and_then(|mut f| {
		if f.path_scope.is_some() {
			// The daemon defaults a missing flag to exact-match.
			f.include_descendants = Some(f.include_descendants.unwrap_or(false));
		} else {
			// Meaningless without a path scope; erase it so it cannot split
			// otherwise-identical subscriptions across two connections.
			f.include_descendants = None;
		}
		let empty = f.library_id.is_none()
			&& f.job_id.is_none()
			&& f.device_id.is_none()
			&& f.resource_type.is_none()
			&& f.path_scope.is_none();
		(!empty).then_some(f)
	});
	(event_types, filter)
}

/// Own one daemon connection for the lifetime of its pool entry: connect,
/// send the `Subscribe` line, forward event frames into the broadcast ring,
/// and reconnect with capped exponential backoff when the connection drops.
/// Exits when the pool entry (and with it the shutdown sender) is dropped.
async fn run_connection(
	socket_addr: String,
	request_line: String,
	events: broadcast::Sender<Event>,
	mut shutdown: watch::Receiver<bool>,
	initial_backoff: Duration,
	max_backoff: Duration,
) {
	let mut backoff = initial_backoff;
	loop {
		match TcpStream::connect(&socket_addr).await {
			Ok(mut stream) => {
				if stream.write_all(request_line.as_bytes()).await.is_ok() {
					// Keep the whole stream (write half included): a closed
					// write direction reads as EOF on the daemon side, which
					// would tear down the subscription.
					let mut lines = BufReader::new(stream).lines();
					loop {
						tokio::select! {
							changed = shutdown.changed() => {
								if changed.is_err() || *shutdown.borrow() {
									return;
								}
							}
							line = lines.next_line() => match line {
								Ok(Some(line)) => {
									match serde_json::from_str::<DaemonResponse>(line.trim()) {
										Ok(DaemonResponse::Event(event)) => {
											// Send only fails with no live
											// receivers; lingering entries
											// simply drop events on the floor.
											let _ = events.send(event);
										}
										Ok(DaemonResponse::Subscribed) => {
											// A healthy session resets the
											// reconnect delay.
											backoff = initial_backoff;
										}
										Ok(DaemonResponse::Error(error)) => {
											tracing::warn!(
												%error,
												"daemon rejected subscription, retrying"
											);
											break;
										}
										Ok(_) => {}
										Err(error) => {
											tracing::warn!(
												%error,
												"unparseable frame on subscription connection"
											);
											break;
										}
									}
								}
								Ok(None) | Err(_) => break,
							}
						}
					}
				}
			}
			Err(error) => {
				tracing::debug!(%error, socket_addr, "subscription connect failed");
			}
		}

		// Disconnected: wait out the backoff unless the entry is dropped.
		tokio::select! {
			changed = shutdown.changed() => {
				if changed.is_err() || *shutdown.borrow() {
					return;
				}
			}
			_ = tokio::time::sleep(backoff) => {}
		}
		backoff = (backoff * 2).min(max_backoff);
	}
}

/// Process linger checks: after each idle notification, wait out the linger
/// period and close the connection if no subscriber reclaimed it. Exits when
/// the broker and all subscription guards are gone.
async fn run_reaper(
	inner: Weak<BrokerInner>,
	mut rx: mpsc::UnboundedReceiver<LingerCheck>,
	linger: Duration,
) {
	while let Some(check) = rx.recv().await {
		let inner = inner.clone();
		tokio::spawn(async move {
			tokio::time::sleep(linger).await;
			let Some(inner) = inner.upgrade() else {
				return;
			};
			let mut state = lock(&inner.state);
			let close = state
				.entries
				.get(&check.key)
				.is_some_and(|e| e.subscriber_count == 0 && e.generation == check.generation);
			if close {
				// Dropping the entry drops its shutdown sender, ending the
				// connection task.
				state.entries.remove(&check.key);
			}
		});
	}
}

/// Lock a mutex, recovering the data if a panic elsewhere poisoned it. The
/// guarded state stays consistent under poisoning: every critical section is
/// a plain field update with no intermediate invalid states.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
	mutex
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn key(event_types: Vec<String>, filter: Option<EventFilter>) -> String {
		let (event_types, filter) = canonicalize(event_types, filter);
		serde_json::to_string(&DaemonRequest::Subscribe {
			event_types,
			filter,
		})
		.unwrap()
	}

	fn empty_filter() -> EventFilter {
		EventFilter {
			library_id: None,
			job_id: None,
			device_id: None,
			resource_type: None,
			path_scope: None,
			include_descendants: None,
		}
	}

	#[test]
	fn event_type_order_and_duplicates_do_not_split_keys() {
		let a = key(vec!["B".into(), "A".into(), "A".into()], None);
		let b = key(vec!["A".into(), "B".into()], None);
		assert_eq!(a, b);
	}

	#[test]
	fn criteria_free_filter_collapses_to_no_filter() {
		assert_eq!(key(vec![], Some(empty_filter())), key(vec![], None));
	}

	#[test]
	fn include_descendants_is_ignored_without_a_path_scope() {
		let mut with_flag = empty_filter();
		with_flag.include_descendants = Some(true);
		assert_eq!(key(vec![], Some(with_flag)), key(vec![], None));
	}

	#[test]
	fn distinct_filters_produce_distinct_keys() {
		let mut by_library = empty_filter();
		by_library.library_id = Some(uuid::Uuid::new_v4());
		assert_ne!(key(vec![], Some(by_library)), key(vec![], None));
	}
}
