//! # Space endpoint event stream
//!
//! The `/v1/spaces/<space>/events` stream tells a mount client which cached
//! listings and blocks to drop. Until a space has its own event log, the feed
//! is held in memory: one ring of recent changes per endpoint, fed by the
//! filesystem watcher and by the writes agents make through this endpoint.
//! Nothing is written to disk per change, so seqs restart with the daemon and
//! a random log id in every event id makes a client that resumes across a
//! restart get `reset` instead of a silent gap.
//!
//! The ring stores absolute paths and each stream maps them to its own grant
//! root, so one feed serves every grant on the endpoint.
//!
//! A stream has no idle timer, because a mount that is up but not reading
//! still needs its invalidations. It closes when a write fails or stalls,
//! when the grant ends, or when the grant opens more streams than its limit,
//! which closes its oldest one. A client that went away without closing its
//! connection therefore never holds a slot for long or locks its grant out.

use std::{
	collections::{HashMap, VecDeque},
	convert::Infallible,
	future::Future,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, AtomicU64, Ordering},
		Arc, Mutex,
	},
	time::Duration,
};

use axum::{
	body::{Body, Bytes},
	http::{header, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
};
use rand::RngCore;
use sd_fs_watcher::{FsEvent, FsEventKind};
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tracing::debug;
use uuid::Uuid;

use super::{
	http::{self, Endpoint},
	paths,
	tools::Tool,
};
use crate::{context::CoreContext, ops::grants::GrantScope};

/// Changes held for replay.
const RING_SIZE: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
	Create,
	Modify,
	Move,
	Delete,
}

impl Op {
	fn name(self) -> &'static str {
		match self {
			Self::Create => "create",
			Self::Modify => "modify",
			Self::Move => "move",
			Self::Delete => "delete",
		}
	}
}

#[derive(Debug)]
pub(super) struct Change {
	seq: u64,
	op: Op,
	path: PathBuf,
	to_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum Signal {
	Change(Arc<Change>),
	/// Changes were lost before reaching the ring.
	Reset,
}

struct Ring {
	next: u64,
	/// The lowest seq a client may resume after: every change past it is held.
	floor: u64,
	changes: VecDeque<Arc<Change>>,
}

/// The in-memory change feed one endpoint serves its streams from.
pub(super) struct Feed {
	log: String,
	ring: Mutex<Ring>,
	live: broadcast::Sender<Signal>,
	watching: AtomicBool,
	/// Each grant's open streams, oldest first.
	streams: Mutex<HashMap<Uuid, VecDeque<OpenStream>>>,
	next_stream: AtomicU64,
}

struct OpenStream {
	id: u64,
	evict: oneshot::Sender<()>,
}

impl Feed {
	pub(super) fn new() -> Self {
		let mut log = [0u8; 6];
		rand::rngs::OsRng.fill_bytes(&mut log);
		Self {
			log: hex::encode(log),
			ring: Mutex::new(Ring {
				next: 1,
				floor: 0,
				changes: VecDeque::new(),
			}),
			live: broadcast::channel(1024).0,
			watching: AtomicBool::new(false),
			streams: Mutex::default(),
			next_stream: AtomicU64::new(0),
		}
	}

	fn push(&self, op: Op, path: PathBuf, to_path: Option<PathBuf>) {
		let mut ring = self.ring.lock().expect("feed lock");
		let change = Arc::new(Change {
			seq: ring.next,
			op,
			path,
			to_path,
		});
		ring.next += 1;
		ring.changes.push_back(change.clone());
		if ring.changes.len() > RING_SIZE {
			if let Some(evicted) = ring.changes.pop_front() {
				ring.floor = ring.floor.max(evicted.seq);
			}
		}
		// Sent under the lock so live subscribers see seqs in order.
		let _ = self.live.send(Signal::Change(change));
	}

	/// Forget every position up to now, after changes were dropped unseen.
	fn lost(&self) {
		let mut ring = self.ring.lock().expect("feed lock");
		ring.changes.clear();
		ring.floor = ring.next;
		let _ = self.live.send(Signal::Reset);
	}

	/// Record what a write tool did, so a mount sees its own writes even
	/// where no watcher covers the root.
	///
	/// `outcome` is the tool's structured output; its paths are relative to
	/// `root`. `replaces` says whether a `write_file` named a precondition,
	/// which makes it a change to an existing file rather than a create.
	pub(super) fn record(&self, tool: Tool, root: &Path, replaces: bool, outcome: &Value) {
		let at = |field: &Value| field.as_str().map(|path| root.join(path));
		let (Some(requested), Some(result)) =
			(at(&outcome["requested"]), at(&outcome["state"]["path"]))
		else {
			return;
		};
		match tool {
			Tool::WriteFile if replaces && requested == result => {
				self.push(Op::Modify, result, None)
			}
			Tool::WriteFile | Tool::MakeDir => self.push(Op::Create, result, None),
			Tool::Move => self.push(Op::Move, requested, Some(result)),
			Tool::Delete => self.push(Op::Delete, requested, None),
			_ => {}
		}
	}

	fn observe(&self, event: &FsEvent) {
		match &event.kind {
			FsEventKind::Create => self.push(Op::Create, event.path.clone(), None),
			FsEventKind::Modify => self.push(Op::Modify, event.path.clone(), None),
			FsEventKind::Remove => self.push(Op::Delete, event.path.clone(), None),
			FsEventKind::Rename { from, to } => self.push(Op::Move, from.clone(), Some(to.clone())),
		}
	}

	/// Start following the filesystem watcher, once one is running.
	pub(super) async fn watch(self: &Arc<Self>, context: &CoreContext) {
		if self.watching.load(Ordering::Acquire) {
			return;
		}
		let Some(watcher) = context.get_fs_watcher().await else {
			return;
		};
		if self.watching.swap(true, Ordering::AcqRel) {
			return;
		}
		let mut events = watcher.subscribe();
		let feed = Arc::downgrade(self);
		tokio::spawn(async move {
			loop {
				let received = events.recv().await;
				let Some(feed) = feed.upgrade() else { return };
				match received {
					Ok(event) => feed.observe(&event),
					Err(broadcast::error::RecvError::Lagged(missed)) => {
						debug!(missed, "event stream feed fell behind the watcher");
						feed.lost();
					}
					Err(broadcast::error::RecvError::Closed) => return,
				}
			}
		});
	}

	/// Claim one of a grant's stream slots, evicting its oldest streams
	/// while it holds `max` or more.
	fn open_stream(self: &Arc<Self>, grant: Uuid, max: usize) -> StreamSlot {
		let (evict, evicted) = oneshot::channel();
		let id = self.next_stream.fetch_add(1, Ordering::Relaxed);
		let mut streams = self.streams.lock().expect("streams lock");
		let open = streams.entry(grant).or_default();
		while open.len() >= max {
			let Some(oldest) = open.pop_front() else {
				break;
			};
			let _ = oldest.evict.send(());
		}
		open.push_back(OpenStream { id, evict });
		StreamSlot {
			feed: self.clone(),
			grant,
			id,
			evicted,
		}
	}

	/// The changes after `last_event_id`, or `None` when they are not all
	/// held and the client must reset.
	fn since(&self, last_event_id: &str) -> Option<Vec<Arc<Change>>> {
		let (log, seq) = last_event_id.trim().rsplit_once('-')?;
		let seq: u64 = seq.parse().ok()?;
		if log != self.log {
			return None;
		}
		let ring = self.ring.lock().expect("feed lock");
		if seq < ring.floor || seq >= ring.next {
			return None;
		}
		Some(
			ring.changes
				.iter()
				.filter(|change| change.seq > seq)
				.cloned()
				.collect(),
		)
	}

	/// `change` as one SSE event for a stream rooted at `roots`, every
	/// spelling of one directory, or `None` when it happened outside it.
	fn frame(&self, change: &Change, roots: &[PathBuf]) -> Option<String> {
		let inside = |path: &Path| {
			roots
				.iter()
				.find_map(|root| path.strip_prefix(root).ok())
				.map(paths::shown)
		};
		let from = inside(&change.path);
		let to = change.to_path.as_deref().and_then(inside);
		let (op, path, to_path) = match (change.op, from, to) {
			(Op::Move, Some(from), Some(to)) => (Op::Move, from, Some(to)),
			(Op::Move, Some(from), None) => (Op::Delete, from, None),
			(Op::Move, None, Some(to)) => (Op::Create, to, None),
			(op, Some(path), _) if op != Op::Move => (op, path, None),
			_ => return None,
		};
		let mut data = json!({ "seq": change.seq, "op": op.name(), "path": path });
		if let Some(to_path) = to_path {
			data["to_path"] = json!(to_path);
		}
		Some(format!(
			"id: {}-{}\nevent: change\ndata: {data}\n\n",
			self.log, change.seq
		))
	}
}

/// A grant's claim on one open stream, given back when the stream ends.
struct StreamSlot {
	feed: Arc<Feed>,
	grant: Uuid,
	id: u64,
	/// Resolves when a newer stream of the grant took this slot.
	evicted: oneshot::Receiver<()>,
}

impl Drop for StreamSlot {
	fn drop(&mut self) {
		let mut streams = self.feed.streams.lock().expect("streams lock");
		if let Some(open) = streams.get_mut(&self.grant) {
			open.retain(|stream| stream.id != self.id);
			if open.is_empty() {
				streams.remove(&self.grant);
			}
		}
	}
}

const RESET: &str = "event: reset\ndata: {}\n\n";
const END: &str = "event: end\ndata: {}\n\n";
const KEEP: &str = ": keep\n\n";

/// Open an event stream for `scope`. At the grant's stream limit the
/// grant's oldest stream gets `reset` and closes to make room.
pub(super) async fn open(
	endpoint: Arc<Endpoint>,
	scope: GrantScope,
	token: String,
	last_event_id: Option<String>,
) -> Response {
	let feed = endpoint.feed.clone();
	feed.watch(&endpoint.context).await;
	let slot = feed.open_stream(scope.grant_id, endpoint.limits.sessions_per_grant);

	// Subscribe before reading the ring so no change falls between them.
	let live = feed.live.subscribe();
	let (reset, replay) = match last_event_id.as_deref().map(|id| feed.since(id)) {
		None => (false, Vec::new()),
		Some(Some(replay)) => (false, replay),
		Some(None) => (true, Vec::new()),
	};

	let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(64);
	let (library, grant) = (scope.library_id, scope.grant_id);
	// Only spellings that reach the granted directory itself, so an alias
	// that names some other directory cannot widen the stream.
	let root = scope.root.clone();
	let roots = tokio::task::spawn_blocking(move || {
		crate::config::path_spellings(&root)
			.into_iter()
			.filter(|spelling| same_file::is_same_file(spelling, &root).unwrap_or(false))
			.collect::<Vec<_>>()
	})
	.await
	.ok()
	.filter(|roots| !roots.is_empty())
	.unwrap_or_else(|| vec![scope.root.clone()]);
	let stream = Stream {
		feed,
		slot,
		roots,
		live,
		reset,
		replay,
		tx,
		heartbeat: endpoint.limits.stream_heartbeat,
		write_timeout: endpoint.limits.stream_write_timeout,
	};
	tokio::spawn(async move {
		stream.run(http::ended(&endpoint, library, &token)).await;
	});

	let mut response = (StatusCode::OK, Body::from_stream(ReceiverStream::new(rx))).into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("text/event-stream"),
	);
	headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
	response
}

/// One open stream's writer.
struct Stream {
	feed: Arc<Feed>,
	slot: StreamSlot,
	/// The grant root under each spelling, since the watcher reports the
	/// spelling the kernel uses, such as /System/Volumes/Data on macOS,
	/// while the grant keeps the one it was made with.
	roots: Vec<PathBuf>,
	live: broadcast::Receiver<Signal>,
	reset: bool,
	replay: Vec<Arc<Change>>,
	tx: mpsc::Sender<Result<Bytes, Infallible>>,
	heartbeat: Duration,
	write_timeout: Duration,
}

impl Stream {
	/// Write events and heartbeats until a write fails or stalls past the
	/// write timeout, a newer stream evicts this one, or `ended` resolves.
	async fn run(self, ended: impl Future<Output = ()>) {
		let Self {
			feed,
			mut slot,
			roots,
			mut live,
			reset,
			replay,
			tx,
			heartbeat,
			write_timeout,
		} = self;
		tokio::pin!(ended);
		let mut sent = replay.last().map_or(0, |change| change.seq);

		// Every write races the grant's end and eviction, and gives up after
		// the write timeout, so a client that stops reading cannot hold the
		// stream or its slot.
		macro_rules! deliver {
			($text:expr) => {
				tokio::select! {
					biased;
					_ = &mut ended => {
						let _ = tx.try_send(Ok(Bytes::from_static(END.as_bytes())));
						return;
					}
					_ = &mut slot.evicted => {
						let _ = tx.try_send(Ok(Bytes::from_static(RESET.as_bytes())));
						return;
					}
					written = tokio::time::timeout(write_timeout, tx.send(Ok(Bytes::from($text)))) => {
						if !matches!(written, Ok(Ok(()))) {
							debug!(grant = %slot.grant, "closing an event stream whose client stopped reading");
							return;
						}
					}
				}
			};
		}

		if reset {
			deliver!(RESET);
		}
		for change in &replay {
			if let Some(frame) = feed.frame(change, &roots) {
				deliver!(frame);
			}
		}

		// A comment every heartbeat keeps intermediaries from closing a quiet stream.
		let mut keep = tokio::time::interval_at(tokio::time::Instant::now() + heartbeat, heartbeat);
		loop {
			let frame = tokio::select! {
				_ = &mut ended => {
					let _ = tx.try_send(Ok(Bytes::from_static(END.as_bytes())));
					return;
				}
				_ = &mut slot.evicted => {
					let _ = tx.try_send(Ok(Bytes::from_static(RESET.as_bytes())));
					return;
				}
				_ = tx.closed() => return,
				_ = keep.tick() => {
					deliver!(KEEP);
					continue;
				}
				signal = live.recv() => match signal {
					Ok(Signal::Change(change)) if change.seq > sent => {
						sent = change.seq;
						match feed.frame(&change, &roots) {
							Some(frame) => frame,
							None => continue,
						}
					}
					Ok(Signal::Change(_)) => continue,
					Ok(Signal::Reset) | Err(broadcast::error::RecvError::Lagged(_)) => RESET.to_string(),
					Err(broadcast::error::RecvError::Closed) => return,
				},
			};
			deliver!(frame);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resumes_only_within_the_ring_and_the_log() {
		let feed = Feed::new();
		feed.push(Op::Create, PathBuf::from("/r/a"), None);
		feed.push(Op::Modify, PathBuf::from("/r/a"), None);
		let log = feed.log.clone();
		assert_eq!(feed.since(&format!("{log}-1")).unwrap().len(), 1);
		assert_eq!(feed.since(&format!("{log}-2")).unwrap().len(), 0);
		assert!(
			feed.since(&format!("{log}-3")).is_none(),
			"a seq from the future"
		);
		assert!(feed.since("000000-1").is_none(), "another log");
		assert!(feed.since("garbage").is_none());
		feed.lost();
		assert!(
			feed.since(&format!("{log}-2")).is_none(),
			"lost changes force a reset"
		);
	}

	fn stream(
		feed: &Arc<Feed>,
		slot: StreamSlot,
		tx: mpsc::Sender<Result<Bytes, Infallible>>,
	) -> Stream {
		Stream {
			feed: feed.clone(),
			slot,
			roots: vec![PathBuf::from("/r")],
			live: feed.live.subscribe(),
			reset: false,
			replay: Vec::new(),
			tx,
			heartbeat: Duration::from_secs(15),
			write_timeout: Duration::from_millis(100),
		}
	}

	#[tokio::test]
	async fn a_stream_whose_client_stops_reading_closes_after_the_write_timeout() {
		let feed = Arc::new(Feed::new());
		let slot = feed.open_stream(Uuid::new_v4(), 4);
		let (tx, _unread) = mpsc::channel(1);
		let task = tokio::spawn(stream(&feed, slot, tx).run(std::future::pending()));
		feed.push(Op::Create, PathBuf::from("/r/a"), None);
		feed.push(Op::Create, PathBuf::from("/r/b"), None);
		tokio::time::timeout(Duration::from_secs(5), task)
			.await
			.expect("the stream closes once a write stalls")
			.unwrap();
		assert!(
			feed.streams.lock().unwrap().is_empty(),
			"the closed stream gives its slot back"
		);
	}

	#[tokio::test]
	async fn a_grant_over_its_stream_limit_evicts_its_oldest_stream() {
		let feed = Arc::new(Feed::new());
		let grant = Uuid::new_v4();
		let (tx, mut rx) = mpsc::channel(8);
		let oldest =
			tokio::spawn(stream(&feed, feed.open_stream(grant, 2), tx).run(std::future::pending()));
		let _second = feed.open_stream(grant, 2);
		let _third = feed.open_stream(grant, 2);
		tokio::time::timeout(Duration::from_secs(5), oldest)
			.await
			.expect("the oldest stream closes")
			.unwrap();
		assert_eq!(
			rx.recv().await.unwrap().unwrap(),
			Bytes::from_static(RESET.as_bytes())
		);
		assert!(rx.recv().await.is_none());
		assert_eq!(feed.streams.lock().unwrap()[&grant].len(), 2);
	}

	#[test]
	fn the_ring_forgets_its_oldest_changes() {
		let feed = Feed::new();
		for _ in 0..RING_SIZE + 5 {
			feed.push(Op::Modify, PathBuf::from("/r/a"), None);
		}
		let log = feed.log.clone();
		assert!(feed.since(&format!("{log}-1")).is_none());
		assert!(feed.since(&format!("{log}-5")).is_some());
	}

	#[test]
	fn frames_are_relative_to_the_stream_root() {
		let feed = Feed::new();
		let root = &[PathBuf::from("/r")][..];
		let change = |op, path: &str, to: Option<&str>| Change {
			seq: 7,
			op,
			path: PathBuf::from(path),
			to_path: to.map(PathBuf::from),
		};
		let frame = feed
			.frame(&change(Op::Modify, "/r/docs/a.txt", None), root)
			.unwrap();
		assert!(frame.contains(r#""path":"docs/a.txt""#), "{frame}");
		assert!(frame.starts_with(&format!("id: {}-7\n", feed.log)));
		assert!(feed
			.frame(&change(Op::Modify, "/elsewhere/a", None), root)
			.is_none());
		let frame = feed
			.frame(&change(Op::Move, "/r/a", Some("/elsewhere/a")), root)
			.unwrap();
		assert!(frame.contains(r#""op":"delete""#), "{frame}");
		let frame = feed
			.frame(&change(Op::Move, "/elsewhere/a", Some("/r/b")), root)
			.unwrap();
		assert!(
			frame.contains(r#""op":"create""#) && frame.contains(r#""path":"b""#),
			"{frame}"
		);
		let frame = feed
			.frame(&change(Op::Move, "/r/a", Some("/r/b/a")), root)
			.unwrap();
		assert!(frame.contains(r#""to_path":"b/a""#), "{frame}");
	}

	#[test]
	fn frames_match_the_root_under_any_spelling() {
		let feed = Feed::new();
		let roots = [
			PathBuf::from("/Users/me/granted"),
			PathBuf::from("/System/Volumes/Data/Users/me/granted"),
		];
		for path in [
			"/Users/me/granted/docs/a.txt",
			"/System/Volumes/Data/Users/me/granted/docs/a.txt",
		] {
			let change = Change {
				seq: 1,
				op: Op::Modify,
				path: PathBuf::from(path),
				to_path: None,
			};
			let frame = feed.frame(&change, &roots).unwrap();
			assert!(frame.contains(r#""path":"docs/a.txt""#), "{frame}");
		}
	}
}
