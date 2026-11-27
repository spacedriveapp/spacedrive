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

use std::{
	collections::{HashMap, VecDeque},
	convert::Infallible,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
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
use tokio::sync::{broadcast, mpsc};
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
/// A comment goes out this often so intermediaries keep the stream open.
const KEEP_ALIVE: Duration = Duration::from_secs(15);
/// A stream that carried no event this long is closed; the client resumes.
const IDLE_CLOSE: Duration = Duration::from_secs(60);

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
	streams: Mutex<HashMap<Uuid, usize>>,
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

	/// Claim one of a grant's stream slots.
	fn open_stream(self: &Arc<Self>, grant: Uuid, max: usize) -> Option<StreamSlot> {
		let mut streams = self.streams.lock().expect("streams lock");
		let open = streams.entry(grant).or_default();
		if *open >= max {
			return None;
		}
		*open += 1;
		Some(StreamSlot {
			feed: self.clone(),
			grant,
		})
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

	/// `change` as one SSE event for a stream rooted at `root`, or `None`
	/// when it happened outside the root.
	fn frame(&self, change: &Change, root: &Path) -> Option<String> {
		let inside = |path: &Path| path.strip_prefix(root).ok().map(paths::shown);
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
}

impl Drop for StreamSlot {
	fn drop(&mut self) {
		let mut streams = self.feed.streams.lock().expect("streams lock");
		if let Some(open) = streams.get_mut(&self.grant) {
			*open = open.saturating_sub(1);
			if *open == 0 {
				streams.remove(&self.grant);
			}
		}
	}
}

const RESET: &str = "event: reset\ndata: {}\n\n";
const END: &str = "event: end\ndata: {}\n\n";
const KEEP: &str = ": keep\n\n";

/// Open an event stream for `scope`, or `None` when the grant already
/// holds its limit of streams.
pub(super) async fn open(
	endpoint: Arc<Endpoint>,
	scope: GrantScope,
	last_event_id: Option<String>,
) -> Option<Response> {
	let feed = endpoint.feed.clone();
	feed.watch(&endpoint.context).await;
	let slot = feed.open_stream(scope.grant_id, endpoint.limits.sessions_per_grant)?;

	// Subscribe before reading the ring so no change falls between them.
	let mut live = feed.live.subscribe();
	let (reset, replay) = match last_event_id.as_deref().map(|id| feed.since(id)) {
		None => (false, Vec::new()),
		Some(Some(replay)) => (false, replay),
		Some(None) => (true, Vec::new()),
	};
	let mut sent = replay.last().map_or(0, |change| change.seq);

	let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(64);
	tokio::spawn(async move {
		let _slot = slot;
		let root = scope.root;
		let send = |text: String| {
			let tx = tx.clone();
			async move { tx.send(Ok(Bytes::from(text))).await.is_ok() }
		};
		if reset && !send(RESET.to_string()).await {
			return;
		}
		for change in &replay {
			if let Some(frame) = feed.frame(change, &root) {
				if !send(frame).await {
					return;
				}
			}
		}

		let ended = http::ended(&endpoint, scope.library_id, scope.grant_id);
		tokio::pin!(ended);
		let idle = tokio::time::sleep(IDLE_CLOSE);
		tokio::pin!(idle);
		let mut keep =
			tokio::time::interval_at(tokio::time::Instant::now() + KEEP_ALIVE, KEEP_ALIVE);
		loop {
			let frame = tokio::select! {
				_ = &mut ended => {
					send(END.to_string()).await;
					return;
				}
				_ = &mut idle => return,
				_ = tx.closed() => return,
				_ = keep.tick() => {
					if !send(KEEP.to_string()).await {
						return;
					}
					continue;
				}
				signal = live.recv() => match signal {
					Ok(Signal::Change(change)) if change.seq > sent => {
						sent = change.seq;
						match feed.frame(&change, &root) {
							Some(frame) => frame,
							None => continue,
						}
					}
					Ok(Signal::Change(_)) => continue,
					Ok(Signal::Reset) | Err(broadcast::error::RecvError::Lagged(_)) => RESET.to_string(),
					Err(broadcast::error::RecvError::Closed) => return,
				},
			};
			if !send(frame).await {
				return;
			}
			idle.as_mut()
				.reset(tokio::time::Instant::now() + IDLE_CLOSE);
		}
	});

	let mut response = (StatusCode::OK, Body::from_stream(ReceiverStream::new(rx))).into_response();
	let headers = response.headers_mut();
	headers.insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("text/event-stream"),
	);
	headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
	Some(response)
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
		let root = Path::new("/r");
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
}
