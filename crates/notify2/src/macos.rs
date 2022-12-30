use core_foundation::{
	array::CFArray,
	base::{CFIndex, FromVoid},
	dictionary::CFDictionary,
	number::CFNumber,
	runloop::{kCFRunLoopBeforeWaiting, kCFRunLoopDefaultMode, CFRunLoop},
	string::CFString,
};
use fsevent_stream::{ffi::*, flags::StreamFlags, observer::create_oneshot_observer};
use std::{
	ffi::c_void,
	fmt, fs, io,
	ops::Deref,
	panic::catch_unwind,
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	thread::{self, JoinHandle},
	time::Duration,
};
use tokio::{
	runtime::Handle,
	sync::{mpsc, oneshot, Mutex},
};

use crate::Event;

struct WatcherInner {
	/// paths we are listening to
	paths: Mutex<Vec<PathBuf>>,
	/// running is a flag used to indicate if the watcher is running or not. If it is true, and the watcher is stopped it will restart. If false, it will drop the thread.
	running: AtomicBool,
	/// start is a channel used to send a signal to the watcher thread to start again. This is to prevent an infinite loop of starting and stopping the watcher when no paths are provided.
	start: Mutex<Option<oneshot::Sender<()>>>,
	/// is a flag used to indicate if we should ignore file system events that are triggered by the current process.
	ignore_current_process_events: bool,
}

/// macOS File System Watcher
pub struct Watcher {
	inner: Arc<WatcherInner>,
	/// handle to the watcher thread. This is used to gracefully shutdown the watcher thread.
	handle: Option<JoinHandle<()>>, // This will never be `None`!!! Option is required for `Drop` impl to work
	/// runloop allows us to control the objc runloop from outside of the main thread. This allows us to stop the runloop for graceful shutdown or to restart it when new paths are added/removed.
	runloop: SendWrapper<CFRunLoop>,
	/// tx is the channel used to send events to the user. We only hold it here so it isn't dropped until the `Watcher` is dropped.
	_tx: mpsc::Sender<Event>,
}

impl fmt::Debug for Watcher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("Watcher")
	}
}

impl Watcher {
	/// Create a new watcher
	/// You must ensure `Watcher` is not dropped until your done with filesystem watching.
	/// The `mpsc::Receiver` will return `None` when the watcher is dropped as per `tokio::sync::mpsc` docs.
	/// `ignore_self` allows you to ignore events that are triggered by the current process.
	pub async fn new(
		mut paths: Vec<PathBuf>,
		ignore_current_process_events: bool,
	) -> (mpsc::Receiver<Event>, Self) {
		let (tx, rx) = mpsc::channel(1024);
		paths.dedup();
		let inner = Arc::new(WatcherInner {
			paths: Mutex::new(paths),
			running: AtomicBool::new(true),
			start: Mutex::new(None),
			ignore_current_process_events,
		});

		let (runloop_tx, runloop_rx) = oneshot::channel();
		let handle = thread::spawn(Self::thread(
			inner.clone(),
			Context { tx: tx.clone() },
			runloop_tx,
		));
		let runloop = runloop_rx.await.expect("receive runloop from worker"); // TODO: Error handling

		(
			rx,
			Self {
				inner,
				handle: Some(handle),
				runloop,
				_tx: tx,
			},
		)
	}

	fn thread(
		inner: Arc<WatcherInner>,
		ctx: Context,
		runloop_tx: oneshot::Sender<SendWrapper<CFRunLoop>>,
	) -> impl FnOnce() + Send + 'static {
		let handle = Handle::current();
		move || {
			let current_runloop = CFRunLoop::get_current();
			let mut runloop_tx = Some(runloop_tx); // `Some(_)` on first request. `None` on subsequent requests.

			if let Some(runloop_tx) = runloop_tx.take() {
				// the calling to CFRunLoopRun will be terminated by CFRunLoopStop call in drop()
				// Safety:
				// - According to the Apple documentation, it's safe to move `CFRef`s across threads.
				//   https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html
				runloop_tx
					.send(unsafe { SendWrapper::new(current_runloop.clone()) })
					.expect("send runloop to stream"); // TODO: Error handling
			}

			let mut flags = kFSEventStreamCreateFlagNoDefer
				| kFSEventStreamCreateFlagFileEvents
				| kFSEventStreamCreateFlagWatchRoot
				| kFSEventStreamCreateFlagUseExtendedData
				| kFSEventStreamCreateFlagUseCFTypes;
			if inner.ignore_current_process_events {
				flags |= kFSEventStreamCreateFlagIgnoreSelf;
			}

			let mut since_when = kFSEventStreamEventIdSinceNow;
			loop {
				// When no paths are set this thread will get in a loop of starting and stopping the stream. This avoids it!
				if handle.block_on(inner.paths.lock()).len() == 0 {
					let (tx, rx) = oneshot::channel();
					{
						let mut start = handle.block_on(inner.start.lock());
						*start = Some(tx);
					}
					handle.block_on(rx).unwrap();
				}

				// Warning: stream_context will be dropped by the FSEvents API. Reusing it will cause UB
				let stream_context = SysFSEventStreamContext::new(ctx.clone(), release_context);
				let mut stream = SysFSEventStream::new(
					cf_ext_with_id_callback,
					&stream_context,
					// Warning: Passing an empty array of paths here is UB but we handle that above
					handle.block_on(inner.paths.lock()).iter(),
					since_when,
					Duration::from_millis(100), // Gives us an extra level of batching and the user probs won't notice
					flags,
				)
				.unwrap(); // TODO: Error handling

				stream.schedule(&current_runloop, unsafe { kCFRunLoopDefaultMode });
				stream.start();

				CFRunLoop::run_current();
				stream.stop();
				stream.invalidate();

				if inner.running.load(Ordering::Relaxed) {
					println!("Restarting!"); // TODO: Debug logs
					let id = unsafe { FSEventsGetCurrentEventId() };
					since_when = id;

					continue;
				} else {
					println!("Shutdown!"); // TODO: Debug logs
					break;
				}
			}
		}
	}

	/// Add paths to the watcher
	pub async fn add_paths(&self, new_paths: Vec<PathBuf>) {
		{
			let mut paths = self.inner.paths.lock().await;
			let mut new_paths = new_paths
				.into_iter()
				.filter(|v| !paths.contains(v))
				.collect::<Vec<_>>();
			new_paths.dedup();
			if new_paths.is_empty() {
				println!("Skip"); // TODO
				return;
			}
			paths.append(&mut new_paths);
		}

		if let Some(start) = self.inner.start.lock().await.take() {
			start.send(()).unwrap();
		} else {
			self.shutdown_inner();
		}
	}

	/// Remove paths from the watcher
	pub async fn remove_paths(&self, remove_paths: Vec<PathBuf>) {
		if remove_paths.is_empty() {
			return;
		}

		{
			let mut paths = self.inner.paths.lock().await;
			let remove_paths = remove_paths
				.into_iter()
				.filter(|v| paths.contains(v))
				.collect::<Vec<_>>();
			if remove_paths.is_empty() {
				println!("Skip"); // TODO

				return;
			}
			paths.retain(|v| !remove_paths.contains(v));
		}

		if let Some(start) = self.inner.start.lock().await.take() {
			start.send(()).unwrap();
		} else {
			self.shutdown_inner();
		}
	}

	fn shutdown_inner(&self) {
		let (tx, rx) = std::sync::mpsc::channel();
		let observer = create_oneshot_observer(kCFRunLoopBeforeWaiting, tx);
		self.runloop
			.add_observer(&observer, unsafe { kCFRunLoopDefaultMode });

		if !self.runloop.is_waiting() {
			// Wait the RunLoop to enter Waiting state.

			// TODO: Sometimes this just never resolves in the `double_remove` test. Debug it further and fix it!

			rx.recv_timeout(Duration::from_secs(4)) // TODO: We should probs use a tokio channel here so it doesn't block an async thread
				.expect("channel to receive BeforeWaiting signal");
		}

		self.runloop
			.remove_observer(&observer, unsafe { kCFRunLoopDefaultMode });
		self.runloop.stop();
	}
}

impl Drop for Watcher {
	// This drop impl does a lot of blocking which is not ideal. Rust async drop could help here but it's not stable yet.
	fn drop(&mut self) {
		println!("DROP WATCHER");

		// Tell the thread not to restart itself and instead drop the thread once done.
		self.inner.running.store(false, Ordering::Relaxed);

		// Trigger the thread to shut down.
		if let Some(start) = self.inner.start.try_lock().ok().and_then(|mut v| v.take()) {
			start.send(()).unwrap();
		} else {
			self.shutdown_inner();
		}

		// Wait for the thread to shut down.
		if let Some(handle) = self.handle.take() {
			handle.join().expect("thread to shut down"); // TODO: error handling
		}
	}
}

/// Context holds data that is passed to the callback function, allowing us to propagate events outward.
#[derive(Debug, Clone)]
struct Context {
	// transmit channel to send events to the user
	tx: mpsc::Sender<Event>,
}

extern "C" fn release_context(ctx: *mut std::ffi::c_void) {
	unsafe {
		println!("CTX {:?}", Box::from_raw(ctx as *mut Context));
		// drop(Box::from_raw(ctx as *mut Context)); // TODO: Make this work so we aren't leaking memory
	}
}

// Trust me bro
struct SendWrapper<T>(T);

impl<T> SendWrapper<T> {
	const unsafe fn new(t: T) -> Self {
		Self(t)
	}
}

unsafe impl<T> Send for SendWrapper<T> {}

impl<T> Deref for SendWrapper<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> fmt::Debug for SendWrapper<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SendWrapper").finish()
	}
}

#[allow(clippy::borrow_interior_mutable_const)]
extern "C" fn cf_ext_with_id_callback(
	_stream_ref: SysFSEventStreamRef,
	info: *mut c_void,
	num_events: usize,                     // size_t numEvents
	paths: *mut c_void,                    // void *eventPaths
	flags: *const FSEventStreamEventFlags, // const FSEventStreamEventFlags eventFlags[]
	_ids: *const FSEventStreamEventId,     // const FSEventStreamEventId eventIds[]
) {
	drop(catch_unwind(move || {
		let ctx = unsafe { &(*(info as *const Context)) };
		let paths = unsafe { CFArray::<CFDictionary<CFString>>::from_void(paths) };

		println!("{:?}", paths.len());

		let mut buf = Vec::with_capacity(2); // 2 because it's most likely we get at most two of these in a single callback
		for idx in 0..num_events {
			let (dict, flags) = unsafe { (paths.get_unchecked(idx as CFIndex), *flags.add(idx)) };

			let path = PathBuf::from(
				(*unsafe {
					CFString::from_void(*dict.get(&*kFSEventStreamEventExtendedDataPathKey))
				})
				.to_string(),
			);
			let flag = StreamFlags::from_bits(flags).unwrap(); // TODO: Error handling

			println!("Received event: {} {}", flag, path.display());

			let event = match flag {
				flag if flag.contains(StreamFlags::OWN_EVENT) => continue,
				flag if flag.contains(StreamFlags::MOUNT) => Event::Mount(path),
				flag if flag.contains(StreamFlags::UNMOUNT) => Event::Unmount(path),
				flag if flag.contains(StreamFlags::ROOT_CHANGED) => Event::RootChange(path),
				flag if flag.contains(StreamFlags::ITEM_REMOVED) => {
					Event::Delete((&flag).try_into().unwrap(), path)
				}
				flag if flag.contains(StreamFlags::ITEM_RENAMED) => {
					// We throw these events into a buffer so we can detect both parts of the rename event
					buf.push((
						unsafe {
							CFNumber::from_void(*dict.get(&*kFSEventStreamEventExtendedFileIDKey))
						}
						.to_i64()
						.unwrap(), // TODO: Error handling
						flag,
						path,
					));
					continue;
				}
				// Warning: The `SteamFlags::INODE_META_MOD` flag is used to differentiate between a file being modified and a file being created.
				// This is an assumption @Oscar has made given it makes sense the last modified time in the inode would only be changed on editing the file.
				flag if flag.contains(StreamFlags::ITEM_CREATED)
					&& !flag.contains(StreamFlags::INODE_META_MOD)
					&& !flag.contains(StreamFlags::ITEM_CHANGE_OWNER) =>
				{
					Event::Create((&flag).try_into().unwrap(), path)
				}
				flag if flag.contains(StreamFlags::ITEM_MODIFIED)
					| flag.contains(StreamFlags::ITEM_CHANGE_OWNER)
					| flag.contains(StreamFlags::INODE_META_MOD)
					| flag.contains(StreamFlags::FINDER_INFO_MOD)
					| flag.contains(StreamFlags::ITEM_CHANGE_OWNER)
					| flag.contains(StreamFlags::ITEM_XATTR_MOD) =>
				{
					Event::Modify((&flag).try_into().unwrap(), path)
				}
				flag if flag.contains(StreamFlags::HISTORY_DONE) => continue,
				_ => {
					println!("Unknown event: {:?}", flag); // TODO: Remove

					continue;
				}
			};

			if let Err(e) = ctx.tx.blocking_send(event) {
				println!("Unable to send event from callback: {}", e); // TODO: Remove
			}
		}

		// We are mutating the buffer while also iterating it so this is weird AF.
		while let Some((inode, flag, path)) = buf.pop() {
			// This code is assuming the earlier event is the `from` path and the later event is the `to` path which seems reasonable.
			let event = match buf
				.iter()
				.position(|(inode2, _, _)| *inode2 == inode)
				.map(|idx| buf.remove(idx))
			{
				Some((_inode2, _flag2, from)) => Event::Move {
					ty: (&flag).try_into().unwrap(),
					from,
					to: path,
				},
				None => {
					// Implementation copied from `fs::try_exists`
					// We do this check because sometimes the listener sends a delete event as a rename event
					match fs::metadata(&path) {
						// File exists
						Ok(_) => Event::Modify((&flag).try_into().unwrap(), path),
						// File deleted
						Err(error) if error.kind() == io::ErrorKind::NotFound => {
							Event::Delete((&flag).try_into().unwrap(), path)
						}
						Err(error) => {
							println!("Unable to get metadata for file: {}", error); // TODO: Remove

							Event::Modify((&flag).try_into().unwrap(), path)
						}
					}
				}
			};

			if let Err(e) = ctx.tx.blocking_send(event) {
				println!("Unable to send event from callback: {}", e); // TODO: Remove
			}
		}
	}));
}
