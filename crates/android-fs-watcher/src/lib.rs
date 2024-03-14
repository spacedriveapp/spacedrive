#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	clippy::expect_used,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::as_conversions,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(unsafe_code, deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use std::{
	borrow::Borrow,
	collections::HashMap,
	fmt,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
};

use inotify::{Inotify, WatchDescriptor, WatchMask};
use tokio::sync::mpsc::{self, Sender};
use tracing::info;

type _Watchers = HashMap<PathBuf, WatchDescriptor>;
type Watchers = Arc<Mutex<_Watchers>>;

pub trait EventHandler: Send + 'static {
	/// Handles an event.
	fn handle_event(&mut self, event: Result<WatcherEvent, std::io::Error>);
}

impl<F> EventHandler for F
where
	F: FnMut(Result<WatcherEvent, std::io::Error>) + Send + 'static,
{
	fn handle_event(&mut self, event: Result<WatcherEvent, std::io::Error>) {
		(self)(event);
	}
}

pub struct AndroidWatcher {
	inotify: Inotify,
	internal_handle: InternalHandle,
	event_handler: Arc<Mutex<dyn EventHandler>>,
}

impl fmt::Debug for AndroidWatcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AndroidWatcher")
			.field("inotify", &self.inotify)
			.field("internal_handle", &self.internal_handle)
			.field("event_handler", &Arc::as_ptr(&self.event_handler))
			.finish()
	}
}

#[derive(Debug)]
pub enum WatcherEvent {
	Modify,
	Create,
	Delete,
}

pub enum InternalEvent {
	AddWatch(PathBuf, WatchDescriptor),
	RemoveWatch(PathBuf),
}

impl AndroidWatcher {
	pub fn init<F: EventHandler>(event_handler: F) -> Result<Self, std::io::Error> {
		Ok(Self::new(Arc::new(Mutex::new(event_handler))))
	}

	fn new(event_handler: Arc<Mutex<dyn EventHandler>>) -> Self {
		let internal_handle = InternalHandle::new();

		Self {
			inotify: Inotify::init().expect("Failed to initialize inotify"),
			internal_handle,
			event_handler,
		}
	}

	pub fn watch(&mut self, path: &Path) -> Result<(), std::io::Error> {
		let wd = self
			.inotify
			.watches()
			.add(
				path.to_path_buf().clone(),
				WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
			)
			.expect("Failed to add watch");

		self.internal_handle
			.send_internal_event(InternalEvent::AddWatch(path.to_path_buf().clone(), wd));

		Ok(())
	}

	pub fn unwatch(&mut self, path: &Path) -> Result<(), std::io::Error> {
		let wd = self
			.internal_handle
			.get_watchers()
			.get(&path.to_path_buf().clone())
			.expect("Failed to get watch descriptor")
			.clone();

		self.inotify
			.watches()
			.remove(wd)
			.expect("Failed to remove watch");

		self.internal_handle
			.send_internal_event(InternalEvent::RemoveWatch(path.to_path_buf().clone()));

		Ok(())
	}
}

#[derive(Debug)]
struct InternalActor {
	receiver: mpsc::Receiver<InternalEvent>,
}

impl InternalActor {
	fn new(receiver: mpsc::Receiver<InternalEvent>) -> Self {
		Self { receiver }
	}

	fn handle_internal_msg(&mut self, msg: InternalEvent) {
		match msg {
			InternalEvent::AddWatch(path, _) => {
				info!("Adding watch for {:?}", path);
			}
			InternalEvent::RemoveWatch(path) => {
				info!("Removing watch for {:?}", path);
			}
		}
	}
}

async fn run_internal_actor(mut actor: InternalActor) {
	while let Some(msg) = actor.receiver.recv().await {
		actor.handle_internal_msg(msg);
		info!("Running?: Yes");
	}
	info!("Running?: ???");
}

#[derive(Debug)]
pub struct InternalHandle {
	sender: mpsc::Sender<InternalEvent>,
	watchers: Watchers,
	join: tokio::task::JoinHandle<()>,
}

impl InternalHandle {
	pub fn new() -> Self {
		let (sender, receiver) = mpsc::channel(128);
		let actor = InternalActor::new(receiver);
		let join = tokio::spawn(run_internal_actor(actor));

		Self {
			sender,
			watchers: Arc::new(Mutex::new(HashMap::new())),
			join,
		}
	}

	async fn _send(sender: Sender<InternalEvent>, event: InternalEvent) {
		sender
			.send(event)
			.await
			.expect("Failed to send event. Task Killed.");
	}

	pub fn send_internal_event(&self, event: InternalEvent) {
		let sender_clone = self.sender.clone();
		tokio::spawn(async move {
			Self::_send(sender_clone, event).await; // Call the associated function directly
		});
		let t = self.join.borrow();
		info!("Join: {:?}", t);
	}

	pub fn get_watchers(&self) -> _Watchers {
		self.watchers.lock().unwrap().clone()
	}
}
