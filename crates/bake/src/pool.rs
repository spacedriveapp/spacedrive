//! The bake pool: a bounded set of worker threads pulling requests off a
//! shared priority queue, running each through a producer chain, and
//! delivering finished tiles over a channel. Priorities let the caller keep
//! the viewport ahead of prefetch; ties complete in submission order.
//!
//! Everything here is synchronous std — the pool is meant to sit under either
//! a blocking caller or an async runtime's channel bridge without dragging an
//! executor into the crate.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::{bake, Decline, Producer, Tile, WorkItem};

/// One unit of work for the pool. `key` is opaque correlation data echoed back
/// on the result (a record uuid, a slot index — the pool doesn't care).
pub struct BakeRequest<K> {
	pub key: K,
	pub item: WorkItem,
	/// Larger bakes first; equal priorities complete in submission order.
	pub priority: u32,
}

/// A completed request: the tile, or every stage's decline in chain order.
pub struct Baked<K> {
	pub key: K,
	pub result: Result<Tile, Vec<Decline>>,
}

/// A fixed-size worker pool over one producer chain and one tile geometry.
///
/// Dropping the pool discards queued requests, lets in-flight bakes finish,
/// and joins the workers.
pub struct BakePool<K> {
	shared: Arc<Shared<K>>,
	workers: Vec<JoinHandle<()>>,
}

struct Shared<K> {
	state: Mutex<State<K>>,
	available: Condvar,
}

struct State<K> {
	queue: BinaryHeap<Entry<K>>,
	next_seq: u64,
	closed: bool,
}

struct Entry<K> {
	priority: u32,
	seq: u64,
	request: BakeRequest<K>,
}

impl<K> PartialEq for Entry<K> {
	fn eq(&self, other: &Self) -> bool {
		self.priority == other.priority && self.seq == other.seq
	}
}

impl<K> Eq for Entry<K> {}

impl<K> PartialOrd for Entry<K> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<K> Ord for Entry<K> {
	fn cmp(&self, other: &Self) -> Ordering {
		// Max-heap: higher priority first, then earlier submission.
		self.priority
			.cmp(&other.priority)
			.then_with(|| other.seq.cmp(&self.seq))
	}
}

impl<K: Send + 'static> BakePool<K> {
	/// Spawn `workers` threads baking through `chain` at `tile_size`. Results
	/// arrive on the returned receiver as they finish.
	pub fn new(
		chain: Vec<Box<dyn Producer>>,
		tile_size: u32,
		workers: usize,
	) -> (Self, Receiver<Baked<K>>) {
		assert!(workers >= 1, "a bake pool needs at least one worker");
		let chain: Arc<[Box<dyn Producer>]> = chain.into();
		let (tx, rx) = mpsc::channel();
		let shared = Arc::new(Shared {
			state: Mutex::new(State {
				queue: BinaryHeap::new(),
				next_seq: 0,
				closed: false,
			}),
			available: Condvar::new(),
		});

		let workers = (0..workers)
			.map(|i| {
				let shared = Arc::clone(&shared);
				let chain = Arc::clone(&chain);
				let tx = tx.clone();
				thread::Builder::new()
					.name(format!("bake-{i}"))
					.spawn(move || worker(&shared, &chain, tile_size, &tx))
					.expect("spawn bake worker")
			})
			.collect();

		(Self { shared, workers }, rx)
	}

	pub fn submit(&self, request: BakeRequest<K>) {
		let mut state = lock(&self.shared.state);
		let seq = state.next_seq;
		state.next_seq += 1;
		state.queue.push(Entry {
			priority: request.priority,
			seq,
			request,
		});
		drop(state);
		self.shared.available.notify_one();
	}
}

impl<K> Drop for BakePool<K> {
	fn drop(&mut self) {
		lock(&self.shared.state).closed = true;
		self.shared.available.notify_all();
		for handle in self.workers.drain(..) {
			let _ = handle.join();
		}
	}
}

fn worker<K>(
	shared: &Shared<K>,
	chain: &[Box<dyn Producer>],
	tile_size: u32,
	tx: &Sender<Baked<K>>,
) {
	loop {
		let request = {
			let mut state = lock(&shared.state);
			loop {
				if state.closed {
					return;
				}
				if let Some(entry) = state.queue.pop() {
					break entry.request;
				}
				state = shared
					.available
					.wait(state)
					.unwrap_or_else(PoisonError::into_inner);
			}
		};
		let result = bake(chain, &request.item, tile_size);
		if tx
			.send(Baked {
				key: request.key,
				result,
			})
			.is_err()
		{
			// The receiver is gone; there is nobody left to deliver to.
			return;
		}
	}
}

fn lock<K>(mutex: &Mutex<State<K>>) -> MutexGuard<'_, State<K>> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	/// Blocks any item whose path is "gate" until released; produces a solid
	/// tile for everything else.
	struct GateProducer {
		gate: Arc<(Mutex<bool>, Condvar)>,
	}

	impl GateProducer {
		fn new() -> (Self, Arc<(Mutex<bool>, Condvar)>) {
			let gate = Arc::new((Mutex::new(false), Condvar::new()));
			(
				Self {
					gate: Arc::clone(&gate),
				},
				gate,
			)
		}
	}

	impl Producer for GateProducer {
		fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
			if item.path.as_os_str() == "gate" {
				let (open, released) = &*self.gate;
				let mut open = open.lock().expect("gate lock");
				while !*open {
					open = released.wait(open).expect("gate wait");
				}
			}
			Ok(Tile::solid(tile_size, [0, 0, 0, 255]))
		}
	}

	fn release(gate: &(Mutex<bool>, Condvar)) {
		*gate.0.lock().expect("gate lock") = true;
		gate.1.notify_all();
	}

	fn request(key: u32, path: &str, priority: u32) -> BakeRequest<u32> {
		BakeRequest {
			key,
			item: WorkItem::file(path),
			priority,
		}
	}

	#[test]
	fn higher_priority_requests_complete_first_when_saturated() {
		let (producer, gate) = GateProducer::new();
		let (pool, rx) = BakePool::new(vec![Box::new(producer) as Box<dyn Producer>], 4, 1);

		// Occupy the single worker so everything below queues up behind it.
		pool.submit(request(0, "gate", 0));
		while !lock(&pool.shared.state).queue.is_empty() {
			thread::yield_now();
		}

		for key in 1..=3 {
			pool.submit(request(key, "prefetch", 1));
		}
		pool.submit(request(10, "viewport", 9));
		pool.submit(request(11, "viewport", 9));
		release(&gate);

		let order: Vec<u32> = (0..6)
			.map(|_| {
				rx.recv_timeout(Duration::from_secs(10))
					.expect("pool delivers")
					.key
			})
			.collect();
		// The gate item first (already claimed), then the viewport pair in
		// submission order, then the prefetch batch in submission order.
		assert_eq!(order, vec![0, 10, 11, 1, 2, 3]);
	}

	#[test]
	fn delivers_declines_for_unbakeable_items() {
		let (pool, rx) = BakePool::<u32>::new(Vec::new(), 4, 2);
		pool.submit(request(7, "anything", 0));
		let baked = rx
			.recv_timeout(Duration::from_secs(10))
			.expect("pool delivers");
		assert_eq!(baked.key, 7);
		assert!(baked.result.is_err());
	}

	#[test]
	fn drop_joins_workers_and_discards_the_queue() {
		let (producer, gate) = GateProducer::new();
		let (pool, rx) = BakePool::new(vec![Box::new(producer) as Box<dyn Producer>], 4, 1);
		pool.submit(request(0, "gate", 0));
		for key in 1..100 {
			pool.submit(request(key, "queued", 0));
		}
		release(&gate);
		drop(pool);
		// Only work claimed before the close is delivered; the rest was
		// discarded, so the channel drains quickly and then disconnects.
		let delivered = rx.iter().count();
		assert!(
			delivered < 100,
			"queued backlog should be discarded on drop"
		);
	}
}
