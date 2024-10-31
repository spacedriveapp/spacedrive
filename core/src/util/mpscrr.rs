//! A multi-producer single-consumer channel (mpsc) with a strongly consistent emit method.
//!
//! What does this mean? Well, any call to [Sender::emit] will not resolve it's future until all active [Receiver]'s have received the value and returned from their callback.
//!
//! Why would you want this? U want to emit a message on a channel and ensure it has been processed by the subscribers before continuing.
//!
//! Things to be aware of:
//!  - Receiver's are lazily registered. Eg. `let rx2 = rx.clone();` will only be required to receive values if [Receiver::subscribe_one] or [Receiver::subscribe] is called on it.
//!  - Panic in a receiver will cause the sender to ignore that receiver. It will not infinitely block on it.
//!
//! ## Example
//!
//! ```rust
//! use sd_core::util::mpscrr;
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let (tx, mut rx) = mpscrr::unbounded_channel::<i32, i32>();
//!
//! tokio::spawn(async move {
//!     rx.subscribe(|value| async move {
//!          assert_eq!(value, 42);
//!
//!          1
//!     })
//!     .await
//!     .unwrap();
//! });
//!
//! // Wait for Tokio to spawn the tasks
//! tokio::time::sleep(std::time::Duration::from_millis(200)).await;
//!
//! let result: Vec<i32> = tx.emit(42).await;
//! assert_eq!(result, vec![1]);
//! # });
//! ```
//!

// We ignore Mutex poising as the code is written such that it will not break any invariants if it panics.
// Keep this in mind!

use std::{
	fmt,
	future::Future,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, PoisonError, RwLock,
	},
};

use futures::future::join_all;
use slotmap::{DefaultKey, SlotMap};
use tokio::sync::{mpsc, oneshot};

pub type Pair<T, U> = (Sender<T, U>, Receiver<T, U>);

type MpscInnerTy<T, U> = (T, oneshot::Sender<U>);

type Slots<T, U> =
	Arc<RwLock<SlotMap<DefaultKey, (mpsc::UnboundedSender<MpscInnerTy<T, U>>, Arc<AtomicBool>)>>>;

enum SenderError {
	/// Receiver was dropped, so remove it.
	Finished(DefaultKey),
	/// Receiver failed to respond but is still assumed active.
	Ignored,
}

/// Returned by a [Receiver] when the [Sender] is dropped while trying to receive a value.
pub struct RecvError {}

impl fmt::Debug for RecvError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("RecvError")
	}
}

#[derive(Debug)]
pub struct Sender<T, U>(Slots<T, U>);

impl<T: Clone, U> Sender<T, U> {
	pub async fn emit(&self, value: T) -> Vec<U> {
		// This is annoying AF but holding a mutex guard over await boundary is `!Sync` which will break code using this.
		let map = self
			.0
			.read()
			.unwrap_or_else(PoisonError::into_inner)
			.iter()
			.map(|(k, v)| (k, v.clone()))
			.collect::<Vec<_>>();

		join_all(map.into_iter().filter_map(|(key, (sender, active))| {
			if !active.load(Ordering::Relaxed) {
				// The receiver has no callback registered so we ignore it.
				return None;
			}

			let value = value.clone();
			Some(async move {
				let (tx, rx) = oneshot::channel();
				if sender.send((value, tx)).is_err() {
					// The receiver was dropped so we remove it from the map
					Err(SenderError::Finished(key))
				} else {
					// If oneshot was dropped we ignore this subscriber as something went wrong with it.
					// It is assumed the mpsc is fine and if it's not it will be cleared up by it's `Drop` or the next `emit`.
					rx.await.map_err(|_| SenderError::Ignored)
				}
			})
		}))
		.await
		.into_iter()
		.filter_map(|x| {
			x.map_err(|e| match e {
				SenderError::Finished(key) => {
					self.0
						.write()
						.unwrap_or_else(PoisonError::into_inner)
						.remove(key);
				}
				SenderError::Ignored => {}
			})
			.ok()
		})
		.collect::<Vec<_>>()
	}
}

#[derive(Debug)]
pub struct Receiver<T, U> {
	slots: Slots<T, U>,
	entry: DefaultKey,
	rx: mpsc::UnboundedReceiver<MpscInnerTy<T, U>>,
	active: Arc<AtomicBool>,
}

impl<T, U> Receiver<T, U> {
	/// This method will call the callback for the next value sent to the channel.
	///
	/// It will block until the next message than return.
	///
	/// If the sender is dropped this will return an error else it will return itself.
	/// This is to avoid using the subscription after the sender is dropped.
	pub async fn subscribe_one<'a, Fu: Future<Output = U> + 'a>(
		mut self,
		func: impl FnOnce(T) -> Fu + 'a,
	) -> Result<Self, RecvError> {
		let _bomb = Bomb::new(&self.active);

		let (value, tx) = self.rx.recv().await.ok_or(RecvError {})?;
		tx.send(func(value).await).map_err(|_| RecvError {})?;

		drop(_bomb);
		Ok(self)
	}

	/// This method will call the callback for every value sent to the channel.
	///
	/// It will block the active task until the sender is dropped.
	///
	/// If the sender is dropped this will return an error.
	pub async fn subscribe<'a, Fu: Future<Output = U> + 'a>(
		mut self,
		mut func: impl FnMut(T) -> Fu + 'a,
	) -> Result<(), RecvError> {
		let _bomb = Bomb::new(&self.active);

		loop {
			let (value, tx) = self.rx.recv().await.ok_or(RecvError {})?;
			tx.send(func(value).await).map_err(|_| RecvError {})?;
		}
	}
}

impl<T, U> Drop for Receiver<T, U> {
	fn drop(&mut self) {
		self.slots
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(self.entry);
	}
}

/// Construct a new unbounded channel.
pub fn unbounded_channel<T, U>() -> (Sender<T, U>, Receiver<T, U>) {
	let mut map = SlotMap::new();

	// Create first receiver
	let (tx, rx) = mpsc::unbounded_channel();
	let active: Arc<AtomicBool> = Arc::default();
	let entry = map.insert((tx, active.clone()));

	let slots = Arc::new(RwLock::new(map));
	(
		Sender(slots.clone()),
		Receiver {
			slots,
			entry,
			rx,
			active,
		},
	)
}

impl<T, U> Clone for Receiver<T, U> {
	fn clone(&self) -> Self {
		let (tx, rx) = mpsc::unbounded_channel();
		let active: Arc<AtomicBool> = Arc::default();
		let entry = self
			.slots
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.insert((tx, active.clone()));

		Self {
			slots: self.slots.clone(),
			entry,
			rx,
			active,
		}
	}
}

// Bomb exists so on panic the `active` flag is reset to false.
struct Bomb<'a>(&'a AtomicBool);

impl<'a> Bomb<'a> {
	pub fn new(b: &'a AtomicBool) -> Self {
		b.store(true, Ordering::Relaxed);
		Self(b)
	}
}

impl Drop for Bomb<'_> {
	fn drop(&mut self) {
		self.0.store(false, Ordering::Relaxed);
	}
}

#[cfg(test)]
mod tests {
	use std::{sync::Arc, time::Duration};

	use boxcar;

	// Not using super because `use super as mpscrr` doesn't work :(
	use crate::util::mpscrr;

	#[derive(Debug, Clone, PartialEq, Eq)]
	enum Step {
		Send,
		RecvA,
		RecvB,
		SendComplete,
	}

	#[tokio::test]
	async fn test_mpscrr() {
		let stack = Arc::new(boxcar::Vec::new());

		let (tx, rx) = mpscrr::unbounded_channel::<u8, u8>();

		tokio::spawn({
			let rx = rx.clone();
			let stack = stack.clone();

			async move {
				rx.subscribe(|value| {
					let stack = stack.clone();

					async move {
						assert_eq!(value, 42);
						stack.push(Step::RecvA);
						1
					}
				})
				.await
				.unwrap();

				// assert!(true, "recv a closed");
			}
		});

		tokio::spawn({
			let rx = rx.clone();
			let stack = stack.clone();

			async move {
				rx.subscribe(|value| {
					let stack = stack.clone();

					async move {
						assert_eq!(value, 42);
						stack.push(Step::RecvB);
						2
					}
				})
				.await
				.unwrap();

				// assert!(true, "recv b closed");
			}
		});

		// Test unsubscribed receiver doesn't cause `.emit` to hang
		let rx3 = rx;

		tokio::time::sleep(Duration::from_millis(200)).await; // Wait for Tokio to spawn the tasks

		stack.push(Step::Send);
		let result = tx.emit(42).await;
		stack.push(Step::SendComplete);
		drop(rx3);

		// Check responses -> U shouldn't should NEVER assume order but we do here for simplicity
		assert_eq!(result, vec![1, 2]);
		// Check the order of operations
		assert_eq!(
			&to_vec(&stack),
			&[Step::Send, Step::RecvA, Step::RecvB, Step::SendComplete,]
		)
	}

	fn to_vec<T: Clone>(a: &boxcar::Vec<T>) -> Vec<T> {
		a.iter().map(|(_, entry)| entry).cloned().collect()
	}
}
