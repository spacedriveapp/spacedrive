use std::{
	borrow::Borrow, collections::HashMap, path::{Path, PathBuf}
};

use inotify::{Inotify, WatchDescriptor, WatchMask};
use tokio::sync::mpsc::{self, Sender};
use tracing::info;

type Watchers = HashMap<PathBuf, WatchDescriptor>;
#[derive(Debug)]
pub struct AndroidWatcher {
	inotify: Inotify,
	internal_handle: InternalHandle,
}

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
	pub fn init() -> Self {
		let internal_handle = InternalHandle::new();

		Self {
			inotify: Inotify::init().expect("Failed to initialize inotify"),
			internal_handle,
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
			.expect("Failed to get watch descriptor").clone();

		self.inotify
			.watches()
			.remove(wd)
			.expect("Failed to remove watch");

		self.internal_handle
			.send_internal_event(InternalEvent::RemoveWatch(path.to_path_buf().clone()));

		Ok(())
	}
	pub fn debug_watches(&self) -> Watchers {
		self.internal_handle.get_watchers()
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

		Self { sender, watchers: HashMap::new(), join }
	}

	async fn _send(sender: Sender<InternalEvent>, event: InternalEvent) {
		sender.send(event).await.expect("Failed to send event. Task Killed.");
	}

	pub fn send_internal_event(&self, event: InternalEvent) {
		let sender_clone = self.sender.clone();
		tokio::spawn(async move {
			Self::_send(sender_clone, event).await; // Call the associated function directly
		});
		let t = self.join.borrow();
		info!("Join: {:?}", t);
	}


	pub fn get_watchers(&self) -> Watchers {
		self.watchers.clone()
	}
}
