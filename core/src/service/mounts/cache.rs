//! Block cache for the byte plane.
//!
//! Streamed ranges land in fixed-size blocks so scrubbing and repeated reads
//! do not cross the network twice. Two tiers: L1 in memory, L2 on disk under
//! `<data_dir>/mounts-cache`, with the provider as L3. The cache is a
//! rebuildable artifact class — deletable at any moment, never
//! authoritative, and rebuilt by being read again.
//!
//! Blocks key on `(source_id, path, source_version, index)`, where the
//! version is derived from the file's size and mtime. A file that changes
//! gets a new version and its old blocks become unreachable, so invalidation
//! is a consequence of the key rather than a mechanism of its own.

use super::provider::{ByteError, ByteProvider, ByteStat, ByteTarget, ProviderClass};
use crate::infra::source_dirs::SourceDirs;
use crate::infra::source_version::source_version;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use uuid::Uuid;

/// Block size. Large enough that a scrub lands inside one block, small
/// enough that a thumbnail read does not drag a whole GOP across the wire.
const BLOCK_BYTES: u64 = 1024 * 1024;

/// Fallback ceiling for the on-disk tier, used when the cache is started
/// without a configured value. The real number comes from `AppConfig`.

/// Share of the disk ceiling held in memory.
const L1_FRACTION: u64 = 64;

/// Speculative fetches allowed in flight across the whole process.
const PREFETCH_SLOTS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BlockKey {
	/// Which source's directory holds the block. Two records can share a
	/// root, so the id — not the path — is what separates them.
	source: Uuid,
	/// Blake3 of (path, version), truncated. Sixteen bytes so a collision
	/// cannot serve one file's bytes for another within a source.
	file: [u8; 16],
	index: u64,
}

impl BlockKey {
	fn file_hex(&self) -> String {
		self.file.iter().map(|b| format!("{b:02x}")).collect()
	}
}

fn file_id(path: &Path, version: u64) -> [u8; 16] {
	let mut hasher = blake3::Hasher::new();
	hasher.update(path.as_os_str().as_encoded_bytes());
	hasher.update(&version.to_le_bytes());
	let mut out = [0u8; 16];
	out.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
	out
}

/// A file's identity for cache purposes. Without an mtime — a peer that
/// cannot report one — size alone still separates most rewrites, and a
/// stale block is bounded by the eviction ceiling rather than being served
/// forever.
fn version_of(stat: &ByteStat) -> u64 {
	match stat.modified {
		Some(mtime) => source_version(stat.size, mtime),
		None => source_version(stat.size, SystemTime::UNIX_EPOCH),
	}
}

// ------------------------------------------------------------------ tiers

/// Byte-capped LRU. Ordering is a monotonic sequence rather than a clock so
/// two touches in the same instant still order.
struct Tier<V> {
	entries: HashMap<BlockKey, (V, u64)>,
	order: BTreeMap<u64, BlockKey>,
	bytes: u64,
	max_bytes: u64,
	next_seq: u64,
}

impl<V> Tier<V> {
	fn new(max_bytes: u64) -> Self {
		Self {
			entries: HashMap::new(),
			order: BTreeMap::new(),
			bytes: 0,
			max_bytes,
			next_seq: 0,
		}
	}

	fn touch(&mut self, key: &BlockKey) {
		let seq = self.next_seq;
		if let Some((_, entry_seq)) = self.entries.get_mut(key) {
			self.order.remove(entry_seq);
			*entry_seq = seq;
			self.order.insert(seq, *key);
			self.next_seq += 1;
		}
	}

	fn insert(&mut self, key: BlockKey, value: V, size: u64) {
		if let Some((_, old_seq)) = self.entries.remove(&key) {
			self.order.remove(&old_seq);
			self.bytes = self.bytes.saturating_sub(size);
		}
		let seq = self.next_seq;
		self.next_seq += 1;
		self.entries.insert(key, (value, seq));
		self.order.insert(seq, key);
		self.bytes += size;
	}

	/// Keys to drop, oldest first, until the tier fits.
	fn overflow(&mut self, block_bytes: u64) -> Vec<BlockKey> {
		let mut evicted = Vec::new();
		while self.bytes > self.max_bytes {
			let Some((&seq, &key)) = self.order.iter().next() else {
				break;
			};
			self.order.remove(&seq);
			self.entries.remove(&key);
			self.bytes = self.bytes.saturating_sub(block_bytes);
			evicted.push(key);
		}
		evicted
	}
}

#[derive(Default)]
pub struct CacheStats {
	pub l1_hits: AtomicU64,
	pub l2_hits: AtomicU64,
	pub misses: AtomicU64,
	pub fetched_bytes: AtomicU64,
	pub served_bytes: AtomicU64,
	pub evicted_blocks: AtomicU64,
}

/// Snapshot of the counters and tier occupancy, for `mounts.cache_status`.
#[derive(Debug, Clone, Default)]
pub struct CacheSnapshot {
	pub l1_bytes: u64,
	pub l1_blocks: u64,
	pub l2_bytes: u64,
	pub l2_blocks: u64,
	pub max_bytes: u64,
	pub block_bytes: u64,
	pub l1_hits: u64,
	pub l2_hits: u64,
	pub misses: u64,
	pub fetched_bytes: u64,
	pub served_bytes: u64,
	pub evicted_blocks: u64,
}

// ------------------------------------------------------------------ cache

pub struct BlockCache {
	dirs: SourceDirs,
	l1: Mutex<Tier<Bytes>>,
	/// L2 holds sizes rather than bytes; the bytes live on disk.
	l2: Mutex<Tier<u64>>,
	/// One fetch per block, however many readers want it at once.
	flight: Mutex<HashMap<BlockKey, Arc<AsyncMutex<()>>>>,
	/// Ceiling on speculative work in flight. Demand reads never take a
	/// permit — a reader waiting on bytes must not queue behind a guess.
	prefetch_slots: Arc<Semaphore>,
	stats: CacheStats,
}

impl BlockCache {
	pub fn new(dirs: SourceDirs, max_bytes: u64) -> Self {
		Self {
			dirs,
			l1: Mutex::new(Tier::new((max_bytes / L1_FRACTION).max(BLOCK_BYTES))),
			l2: Mutex::new(Tier::new(max_bytes)),
			flight: Mutex::new(HashMap::new()),
			prefetch_slots: Arc::new(Semaphore::new(PREFETCH_SLOTS)),
			stats: CacheStats::default(),
		}
	}

	/// Rebuild the disk ledger by walking what is already there, ordered by
	/// write time so a restart does not start from an empty cache. A cache
	/// directory that was deleted between sessions simply comes back empty.
	pub async fn restore(&self) {
		let dirs = self.dirs.clone();
		let found = tokio::task::spawn_blocking(move || {
			let mut found = Vec::new();
			for source in dirs.source_ids() {
				found.extend(scan_blocks(source, &dirs.blocks_dir(source)));
			}
			found
		})
		.await
		.unwrap_or_default();
		let mut l2 = self.l2.lock().unwrap();
		for (key, size) in found {
			l2.insert(key, size, size);
		}
	}

	fn block_path(&self, key: &BlockKey) -> PathBuf {
		let hex = key.file_hex();
		self.dirs
			.blocks_dir(key.source)
			.join(&hex[0..2])
			.join(&hex[2..4])
			.join(format!("{hex}-{:08x}.blk", key.index))
	}

	fn get_l1(&self, key: &BlockKey) -> Option<Bytes> {
		let mut l1 = self.l1.lock().unwrap();
		let hit = l1.entries.get(key).map(|(bytes, _)| bytes.clone());
		if hit.is_some() {
			l1.touch(key);
		}
		hit
	}

	async fn get_l2(&self, key: &BlockKey) -> Option<Bytes> {
		let present = self.l2.lock().unwrap().entries.contains_key(key);
		if !present {
			return None;
		}
		match tokio::fs::read(self.block_path(key)).await {
			Ok(bytes) => {
				self.l2.lock().unwrap().touch(key);
				Some(Bytes::from(bytes))
			}
			// The file vanished under us — a manual delete, or a sweep from
			// another process. Drop the ledger entry and treat it as a miss.
			Err(_) => {
				let mut l2 = self.l2.lock().unwrap();
				if let Some((size, seq)) = l2.entries.remove(key) {
					l2.order.remove(&seq);
					l2.bytes = l2.bytes.saturating_sub(size);
				}
				None
			}
		}
	}

	async fn store(&self, key: BlockKey, bytes: &Bytes) {
		{
			let mut l1 = self.l1.lock().unwrap();
			l1.insert(key, bytes.clone(), bytes.len() as u64);
			let evicted = l1.overflow(BLOCK_BYTES);
			drop(l1);
			let _ = evicted;
		}

		let path = self.block_path(&key);
		if let Some(parent) = path.parent() {
			if tokio::fs::create_dir_all(parent).await.is_err() {
				return;
			}
		}
		// Write through a sibling temp so a torn write is never readable as
		// a block; a failure anywhere here just means no L2 entry.
		let tmp = path.with_extension("part");
		if tokio::fs::write(&tmp, bytes.as_ref()).await.is_err() {
			let _ = tokio::fs::remove_file(&tmp).await;
			return;
		}
		if tokio::fs::rename(&tmp, &path).await.is_err() {
			let _ = tokio::fs::remove_file(&tmp).await;
			return;
		}

		let evicted = {
			let mut l2 = self.l2.lock().unwrap();
			l2.insert(key, bytes.len() as u64, bytes.len() as u64);
			l2.overflow(BLOCK_BYTES)
		};
		for key in evicted {
			self.stats.evicted_blocks.fetch_add(1, Ordering::Relaxed);
			let _ = tokio::fs::remove_file(self.block_path(&key)).await;
		}
	}

	/// Whether a block is already resident or already being fetched, so
	/// read-ahead can skip work someone else is doing.
	fn pending_or_present(&self, key: &BlockKey) -> bool {
		if self.l1.lock().unwrap().entries.contains_key(key) {
			return true;
		}
		if self.l2.lock().unwrap().entries.contains_key(key) {
			return true;
		}
		self.flight.lock().unwrap().contains_key(key)
	}

	fn flight_gate(&self, key: BlockKey) -> Arc<AsyncMutex<()>> {
		let mut flight = self.flight.lock().unwrap();
		flight.entry(key).or_default().clone()
	}

	fn release_gate(&self, key: &BlockKey, gate: Arc<AsyncMutex<()>>) {
		let mut flight = self.flight.lock().unwrap();
		// Two strong refs left (ours and the map's) means nobody else is
		// waiting on this block.
		if Arc::strong_count(&gate) <= 2 {
			flight.remove(key);
		}
	}

	pub fn snapshot(&self) -> CacheSnapshot {
		let l1 = self.l1.lock().unwrap();
		let l2 = self.l2.lock().unwrap();
		CacheSnapshot {
			l1_bytes: l1.bytes,
			l1_blocks: l1.entries.len() as u64,
			l2_bytes: l2.bytes,
			l2_blocks: l2.entries.len() as u64,
			max_bytes: l2.max_bytes,
			block_bytes: BLOCK_BYTES,
			l1_hits: self.stats.l1_hits.load(Ordering::Relaxed),
			l2_hits: self.stats.l2_hits.load(Ordering::Relaxed),
			misses: self.stats.misses.load(Ordering::Relaxed),
			fetched_bytes: self.stats.fetched_bytes.load(Ordering::Relaxed),
			served_bytes: self.stats.served_bytes.load(Ordering::Relaxed),
			evicted_blocks: self.stats.evicted_blocks.load(Ordering::Relaxed),
		}
	}

	/// Change the on-disk ceiling and evict down to it immediately, so a
	/// lowered limit takes effect without waiting for the next write.
	pub async fn set_max_bytes(&self, max_bytes: u64) {
		let evicted = {
			let mut l2 = self.l2.lock().unwrap();
			l2.max_bytes = max_bytes;
			l2.overflow(BLOCK_BYTES)
		};
		{
			let mut l1 = self.l1.lock().unwrap();
			l1.max_bytes = (max_bytes / L1_FRACTION).max(BLOCK_BYTES);
			l1.overflow(BLOCK_BYTES);
		}
		for key in evicted {
			self.stats.evicted_blocks.fetch_add(1, Ordering::Relaxed);
			let _ = tokio::fs::remove_file(self.block_path(&key)).await;
		}
	}

	/// Drop everything in memory, and on disk too when asked. Safe at any
	/// moment: the next read refetches.
	pub async fn clear(&self, include_disk: bool) {
		{
			let mut l1 = self.l1.lock().unwrap();
			let max = l1.max_bytes;
			*l1 = Tier::new(max);
		}
		if !include_disk {
			return;
		}
		{
			let mut l2 = self.l2.lock().unwrap();
			let max = l2.max_bytes;
			*l2 = Tier::new(max);
		}
		for source in self.dirs.source_ids() {
			let _ = tokio::fs::remove_dir_all(self.dirs.blocks_dir(source)).await;
		}
	}
}

fn scan_blocks(source: Uuid, dir: &Path) -> Vec<(BlockKey, u64)> {
	let mut found: Vec<(SystemTime, BlockKey, u64)> = Vec::new();
	let Ok(shards) = std::fs::read_dir(dir) else {
		return Vec::new();
	};
	for shard in shards.flatten() {
		let Ok(inner) = std::fs::read_dir(shard.path()) else {
			continue;
		};
		for sub in inner.flatten() {
			let Ok(blocks) = std::fs::read_dir(sub.path()) else {
				continue;
			};
			for block in blocks.flatten() {
				let path = block.path();
				if path.extension().and_then(|e| e.to_str()) != Some("blk") {
					// A ".part" left by a write interrupted at shutdown.
					let _ = std::fs::remove_file(&path);
					continue;
				}
				let Some(key) = parse_block_name(source, &path) else {
					continue;
				};
				let Ok(meta) = block.metadata() else { continue };
				let written = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
				found.push((written, key, meta.len()));
			}
		}
	}
	found.sort_by_key(|(written, _, _)| *written);
	found
		.into_iter()
		.map(|(_, key, size)| (key, size))
		.collect()
}

fn parse_block_name(source: Uuid, path: &Path) -> Option<BlockKey> {
	let stem = path.file_stem()?.to_str()?;
	let (hex, index) = stem.split_once('-')?;
	if hex.len() != 32 {
		return None;
	}
	let mut file = [0u8; 16];
	for (i, byte) in file.iter_mut().enumerate() {
		*byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
	}
	Some(BlockKey {
		source,
		file,
		index: u64::from_str_radix(index, 16).ok()?,
	})
}

// -------------------------------------------------------------- decorator

/// How far ahead of a sequential reader to fetch. A LAN peer needs a couple
/// of blocks to hide its round trip; a cloud volume needs a deeper pipeline
/// because each fetch costs more latency.
#[derive(Debug, Clone, Copy)]
pub struct ReadAheadPolicy {
	/// Blocks fetched ahead on the first sequential read, and the depth a
	/// seek falls back to.
	start_depth: u64,
	/// Ceiling the depth ramps to while a reader stays sequential.
	max_depth: u64,
	/// Speculative bytes one response may spend, so a scrub through a
	/// 24 GB file never pulls the whole thing.
	budget_bytes: u64,
}

impl ReadAheadPolicy {
	fn for_class(class: ProviderClass) -> Self {
		match class {
			ProviderClass::Peer => Self {
				start_depth: 2,
				max_depth: 8,
				budget_bytes: 64 * 1024 * 1024,
			},
			ProviderClass::Cloud => Self {
				start_depth: 2,
				max_depth: 16,
				budget_bytes: 128 * 1024 * 1024,
			},
			// Local providers are never wrapped; the policy is inert.
			ProviderClass::Local => Self {
				start_depth: 0,
				max_depth: 0,
				budget_bytes: 0,
			},
		}
	}
}

#[derive(Default)]
struct AheadState {
	/// End of the last range served, for telling a sequential reader from a
	/// seek without the frontend having to say which it is.
	last_end: u64,
	depth: u64,
	spent: u64,
}

/// Fetch one run of consecutive blocks in a single provider call and store
/// each block. Gated on the run's first block so concurrent readers of the
/// same region collapse into one fetch, with a re-check inside the gate.
async fn fill_run(
	inner: Arc<dyn ByteProvider>,
	cache: Arc<BlockCache>,
	target: ByteTarget,
	source: Uuid,
	file: [u8; 16],
	run_first: u64,
	run_count: u64,
	size: u64,
) -> Result<Vec<(BlockKey, Bytes)>, ByteError> {
	let gate_key = BlockKey {
		source,
		file,
		index: run_first,
	};
	let gate = cache.flight_gate(gate_key);
	let held = gate.lock().await;

	// Someone may have filled the head of this run while we waited on the
	// gate. Take what is now resident and fetch only the rest.
	let mut out = Vec::new();
	let mut head = 0u64;
	while head < run_count {
		let key = BlockKey {
			source,
			file,
			index: run_first + head,
		};
		if let Some(bytes) = cache.get_l1(&key) {
			cache.stats.l1_hits.fetch_add(1, Ordering::Relaxed);
			out.push((key, bytes));
		} else if let Some(bytes) = cache.get_l2(&key).await {
			cache.stats.l2_hits.fetch_add(1, Ordering::Relaxed);
			cache.store_l1_only(key, &bytes);
			out.push((key, bytes));
		} else {
			break;
		}
		head += 1;
	}

	let fetch_first = run_first + head;
	let fetch_count = run_count - head;
	if fetch_count == 0 {
		drop(held);
		cache.release_gate(&gate_key, gate);
		return Ok(out);
	}

	let start = fetch_first * BLOCK_BYTES;
	let end = (start + fetch_count * BLOCK_BYTES).min(size);
	let result = inner.read_range(&target, start..end).await;

	drop(held);
	cache.release_gate(&gate_key, gate);

	let bytes = result?;
	cache
		.stats
		.fetched_bytes
		.fetch_add(bytes.len() as u64, Ordering::Relaxed);
	cache.stats.misses.fetch_add(fetch_count, Ordering::Relaxed);

	let mut offset = 0u64;
	while offset < bytes.len() as u64 {
		let take = BLOCK_BYTES.min(bytes.len() as u64 - offset);
		let key = BlockKey {
			source,
			file,
			index: fetch_first + offset / BLOCK_BYTES,
		};
		let block = bytes.slice(offset as usize..(offset + take) as usize);
		cache.store(key, &block).await;
		out.push((key, block));
		offset += take;
	}
	Ok(out)
}

/// A provider with the block cache in front of it. Applied to peer and
/// cloud providers by the resolver; local providers are left alone, because
/// their bytes are already on a local disk.
pub struct CachedProvider {
	inner: Arc<dyn ByteProvider>,
	cache: Arc<BlockCache>,
	/// One stat per open, not one per block.
	stat: tokio::sync::OnceCell<ByteStat>,
	policy: ReadAheadPolicy,
	ahead: Mutex<AheadState>,
	/// Speculative work belonging to this response, aborted when the reader
	/// goes away.
	tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl Drop for CachedProvider {
	fn drop(&mut self) {
		// The reader stopped — a player closed the stream, a scrub moved on.
		// Anything still being guessed at is now waste.
		if let Ok(tasks) = self.tasks.lock() {
			for task in tasks.iter() {
				task.abort();
			}
		}
	}
}

impl CachedProvider {
	pub fn new(inner: Arc<dyn ByteProvider>, cache: Arc<BlockCache>) -> Self {
		let policy = ReadAheadPolicy::for_class(inner.class());
		Self {
			inner,
			cache,
			stat: tokio::sync::OnceCell::new(),
			policy,
			ahead: Mutex::new(AheadState::default()),
			tasks: Mutex::new(Vec::new()),
		}
	}

	async fn stat_cached(&self, target: &ByteTarget) -> Result<ByteStat, ByteError> {
		match self.stat.get() {
			Some(stat) => Ok(*stat),
			None => {
				let stat = self.inner.stat(target).await?;
				let _ = self.stat.set(stat);
				Ok(stat)
			}
		}
	}

	/// Blocks this provider will fetch in one call, from the inner
	/// provider's own ceiling.
	fn run_blocks(&self) -> u64 {
		(self.inner.max_read() / BLOCK_BYTES).max(1)
	}

	/// Decide how far ahead to run, and queue it. Sequential readers ramp up
	/// to the policy ceiling; a seek drops back to the starting depth.
	fn arm_read_ahead(
		&self,
		target: &ByteTarget,
		source: Uuid,
		file: [u8; 16],
		range: &Range<u64>,
		size: u64,
	) {
		if self.policy.max_depth == 0 {
			return;
		}

		let (depth, budget_left) = {
			let mut ahead = self.ahead.lock().unwrap();
			if range.start == ahead.last_end && ahead.depth > 0 {
				ahead.depth = (ahead.depth * 2).min(self.policy.max_depth);
			} else {
				ahead.depth = self.policy.start_depth;
			}
			ahead.last_end = range.end;
			(
				ahead.depth,
				self.policy.budget_bytes.saturating_sub(ahead.spent),
			)
		};
		if depth == 0 || budget_left == 0 {
			return;
		}

		let last_block = (range.end.saturating_sub(1)) / BLOCK_BYTES;
		let final_block = size.saturating_sub(1) / BLOCK_BYTES;
		let mut queued = 0u64;

		let mut handles = Vec::new();
		let run = self.run_blocks();
		let mut next = last_block + 1;
		while queued < depth && next <= final_block {
			let count = run.min(depth - queued).min(final_block - next + 1);
			let cost = count * BLOCK_BYTES;
			if cost > budget_left.saturating_sub(queued * BLOCK_BYTES) {
				break;
			}
			// Skip a run whose head is already resident or already coming.
			if self.cache.pending_or_present(&BlockKey {
				source,
				file,
				index: next,
			}) {
				next += count;
				queued += count;
				continue;
			}

			let inner = self.inner.clone();
			let cache = self.cache.clone();
			let target = target.clone();
			let slots = self.cache.prefetch_slots.clone();
			handles.push(tokio::spawn(async move {
				let Ok(_permit) = slots.acquire_owned().await else {
					return;
				};
				let _ = fill_run(inner, cache, target, source, file, next, count, size).await;
			}));
			next += count;
			queued += count;
		}

		if queued > 0 {
			let mut ahead = self.ahead.lock().unwrap();
			ahead.spent += queued * BLOCK_BYTES;
		}
		if !handles.is_empty() {
			let mut tasks = self.tasks.lock().unwrap();
			tasks.retain(|task| !task.is_finished());
			tasks.extend(handles);
		}
	}
}

impl BlockCache {
	/// Promote an L2 hit into memory without rewriting the disk copy.
	fn store_l1_only(&self, key: BlockKey, bytes: &Bytes) {
		let mut l1 = self.l1.lock().unwrap();
		l1.insert(key, bytes.clone(), bytes.len() as u64);
		l1.overflow(BLOCK_BYTES);
	}
}

#[async_trait]
impl ByteProvider for CachedProvider {
	async fn stat(&self, target: &ByteTarget) -> Result<ByteStat, ByteError> {
		self.stat_cached(target).await
	}

	async fn read_range(&self, target: &ByteTarget, range: Range<u64>) -> Result<Bytes, ByteError> {
		let want = range.end.saturating_sub(range.start);
		if want == 0 {
			return Ok(Bytes::new());
		}
		let stat = self.stat_cached(target).await?;
		if range.start >= stat.size {
			return Ok(Bytes::new());
		}
		let end = range.end.min(stat.size);
		let version = version_of(&stat);
		let file = file_id(&target.path, version);
		let source = target.source_id;

		let first = range.start / BLOCK_BYTES;
		let last = (end - 1) / BLOCK_BYTES;

		// Resolve what is already here, and note the gaps.
		let mut blocks: HashMap<u64, Bytes> = HashMap::new();
		let mut missing: Vec<u64> = Vec::new();
		for index in first..=last {
			let key = BlockKey {
				source,
				file,
				index,
			};
			if let Some(bytes) = self.cache.get_l1(&key) {
				self.cache.stats.l1_hits.fetch_add(1, Ordering::Relaxed);
				blocks.insert(index, bytes);
			} else if let Some(bytes) = self.cache.get_l2(&key).await {
				self.cache.stats.l2_hits.fetch_add(1, Ordering::Relaxed);
				self.cache.store_l1_only(key, &bytes);
				blocks.insert(index, bytes);
			} else {
				missing.push(index);
			}
		}

		// Consecutive gaps become one provider call each, capped by what the
		// provider will serve in a single read.
		if !missing.is_empty() {
			let run = self.run_blocks();
			let mut runs: Vec<(u64, u64)> = Vec::new();
			for index in missing {
				match runs.last_mut() {
					Some((start, count)) if *start + *count == index && *count < run => {
						*count += 1;
					}
					_ => runs.push((index, 1)),
				}
			}

			let fetches = runs.into_iter().map(|(start, count)| {
				fill_run(
					self.inner.clone(),
					self.cache.clone(),
					target.clone(),
					source,
					file,
					start,
					count,
					stat.size,
				)
			});
			for filled in futures::future::join_all(fetches).await {
				for (key, bytes) in filled? {
					blocks.insert(key.index, bytes);
				}
			}
		}

		let mut out = BytesMut::with_capacity((end - range.start) as usize);
		for index in first..=last {
			let Some(block) = blocks.get(&index) else {
				break;
			};
			let block_start = index * BLOCK_BYTES;
			let from = range
				.start
				.saturating_sub(block_start)
				.min(block.len() as u64);
			let to = (end - block_start).min(block.len() as u64);
			if to > from {
				out.extend_from_slice(&block[from as usize..to as usize]);
			}
			// A short block is the end of the file; nothing follows it.
			if (block.len() as u64) < BLOCK_BYTES {
				break;
			}
		}

		self.cache
			.stats
			.served_bytes
			.fetch_add(out.len() as u64, Ordering::Relaxed);
		self.arm_read_ahead(target, source, file, &range, stat.size);
		Ok(out.freeze())
	}

	fn class(&self) -> ProviderClass {
		self.inner.class()
	}

	fn max_read(&self) -> u64 {
		self.inner.max_read()
	}
}

// ----------------------------------------------------------------- global

static CACHE: OnceLock<Arc<BlockCache>> = OnceLock::new();

/// Bring the cache up for the life of the process and restore its ledger.
pub async fn init(dirs: SourceDirs, max_bytes: u64) -> Arc<BlockCache> {
	let cache = Arc::new(BlockCache::new(dirs, max_bytes));
	cache.restore().await;
	CACHE.get_or_init(|| cache).clone()
}

pub fn cache() -> Option<Arc<BlockCache>> {
	CACHE.get().cloned()
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::AtomicUsize;

	/// Counts how many times bytes were actually pulled from the far end.
	struct CountingProvider {
		data: Bytes,
		reads: Arc<AtomicUsize>,
		modified: Option<SystemTime>,
	}

	#[async_trait]
	impl ByteProvider for CountingProvider {
		async fn stat(&self, _t: &ByteTarget) -> Result<ByteStat, ByteError> {
			Ok(ByteStat {
				size: self.data.len() as u64,
				modified: self.modified,
			})
		}

		async fn read_range(&self, _t: &ByteTarget, range: Range<u64>) -> Result<Bytes, ByteError> {
			self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
			let start = (range.start as usize).min(self.data.len());
			let end = (range.end as usize).min(self.data.len());
			Ok(self.data.slice(start..end))
		}

		fn class(&self) -> ProviderClass {
			ProviderClass::Peer
		}

		fn max_read(&self) -> u64 {
			4 * 1024 * 1024
		}
	}

	fn payload(len: usize) -> Bytes {
		Bytes::from((0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>())
	}

	fn target() -> ByteTarget {
		ByteTarget {
			source_id: Uuid::from_u128(7),
			path: PathBuf::from("/footage/a001.braw"),
		}
	}

	fn harness(
		dir: &tempfile::TempDir,
		data: Bytes,
	) -> (Arc<AtomicUsize>, Arc<BlockCache>, CachedProvider) {
		let reads = Arc::new(AtomicUsize::new(0));
		let cache = Arc::new(BlockCache::new(
			SourceDirs::new(dir.path().to_path_buf()).unwrap(),
			64 * BLOCK_BYTES,
		));
		let inner = Arc::new(CountingProvider {
			data,
			reads: reads.clone(),
			modified: Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000)),
		});
		let provider = CachedProvider::new(inner, cache.clone());
		(reads, cache, provider)
	}

	#[tokio::test]
	async fn second_read_of_the_same_range_costs_nothing() {
		let dir = tempfile::tempdir().unwrap();
		let data = payload(3 * BLOCK_BYTES as usize);
		let (reads, _cache, provider) = harness(&dir, data.clone());
		let t = target();

		let first = provider.read_range(&t, 0..2048).await.unwrap();
		assert_eq!(&first[..], &data[0..2048]);
		let after_first = reads.load(std::sync::atomic::Ordering::SeqCst);
		assert_eq!(after_first, 1, "one block fetched");

		let second = provider.read_range(&t, 0..2048).await.unwrap();
		assert_eq!(&second[..], &data[0..2048]);
		assert_eq!(
			reads.load(std::sync::atomic::Ordering::SeqCst),
			after_first,
			"served from cache, no further fetch"
		);

		// A backward scrub inside the same block is also free.
		let back = provider.read_range(&t, 512..1024).await.unwrap();
		assert_eq!(&back[..], &data[512..1024]);
		assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), after_first);
	}

	#[tokio::test]
	async fn ranges_spanning_blocks_are_byte_exact() {
		let dir = tempfile::tempdir().unwrap();
		let data = payload(3 * BLOCK_BYTES as usize + 1234);
		let (_reads, _cache, provider) = harness(&dir, data.clone());
		let t = target();

		let start = BLOCK_BYTES - 100;
		let end = 2 * BLOCK_BYTES + 100;
		let got = provider.read_range(&t, start..end).await.unwrap();
		assert_eq!(&got[..], &data[start as usize..end as usize]);

		let whole = provider.read_range(&t, 0..data.len() as u64).await.unwrap();
		assert_eq!(&whole[..], &data[..]);

		let tail_start = 3 * BLOCK_BYTES;
		let tail = provider
			.read_range(&t, tail_start..data.len() as u64)
			.await
			.unwrap();
		assert_eq!(&tail[..], &data[tail_start as usize..]);
	}

	#[tokio::test]
	async fn concurrent_readers_of_one_block_fetch_once() {
		let dir = tempfile::tempdir().unwrap();
		// One block exactly, so read-ahead has nothing to add and the count
		// measures only what single-flight did.
		let data = payload(BLOCK_BYTES as usize);
		let (reads, _cache, provider) = harness(&dir, data.clone());
		let provider = Arc::new(provider);

		let mut handles = Vec::new();
		for offset in [0u64, 100, 4096, 65536, 900_000] {
			let provider = provider.clone();
			handles.push(tokio::spawn(async move {
				provider.read_range(&target(), offset..offset + 256).await
			}));
		}
		for handle in handles {
			handle.await.unwrap().unwrap();
		}
		assert_eq!(
			reads.load(std::sync::atomic::Ordering::SeqCst),
			1,
			"single-flight collapsed five readers into one fetch"
		);
	}

	#[tokio::test]
	async fn sequential_reads_pull_ahead() {
		let dir = tempfile::tempdir().unwrap();
		let data = payload(16 * BLOCK_BYTES as usize);
		let (_reads, cache, provider) = harness(&dir, data.clone());
		let t = target();

		// Read the first block, then the second: the second read starts
		// where the first ended, which is what marks the reader sequential.
		provider.read_range(&t, 0..1024).await.unwrap();
		provider.read_range(&t, 1024..BLOCK_BYTES).await.unwrap();
		provider
			.read_range(&t, BLOCK_BYTES..BLOCK_BYTES + 1024)
			.await
			.unwrap();

		// Give the spawned prefetches a moment to land.
		for _ in 0..50 {
			if cache.snapshot().l2_blocks > 2 {
				break;
			}
			tokio::time::sleep(std::time::Duration::from_millis(10)).await;
		}
		let snap = cache.snapshot();
		assert!(
			snap.l2_blocks > 2,
			"expected blocks beyond the ones read, got {}",
			snap.l2_blocks
		);
		assert!(
			snap.fetched_bytes > snap.served_bytes,
			"read-ahead should pull more than it serves"
		);
	}

	#[tokio::test]
	async fn a_changed_file_does_not_serve_stale_blocks() {
		let dir = tempfile::tempdir().unwrap();
		let t = target();

		let (_r1, cache, provider) = harness(&dir, payload(BLOCK_BYTES as usize));
		let first = provider.read_range(&t, 0..64).await.unwrap();

		// Same path, new mtime and contents: a different version, so a
		// different key, so no chance of the old block answering.
		let changed = Bytes::from(vec![0xABu8; BLOCK_BYTES as usize]);
		let inner = Arc::new(CountingProvider {
			data: changed.clone(),
			reads: Arc::new(AtomicUsize::new(0)),
			modified: Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2000)),
		});
		let provider = CachedProvider::new(inner, cache);
		let second = provider.read_range(&t, 0..64).await.unwrap();

		assert_ne!(&first[..], &second[..]);
		assert_eq!(&second[..], &changed[0..64]);
	}

	#[tokio::test]
	async fn disk_tier_survives_a_restart() {
		let dir = tempfile::tempdir().unwrap();
		let data = payload(2 * BLOCK_BYTES as usize);
		let t = target();

		{
			let (_reads, _cache, provider) = harness(&dir, data.clone());
			provider.read_range(&t, 0..4096).await.unwrap();
		}

		// A fresh cache over the same directory, as after a daemon restart.
		let reads = Arc::new(AtomicUsize::new(0));
		let cache = Arc::new(BlockCache::new(
			SourceDirs::new(dir.path().to_path_buf()).unwrap(),
			64 * BLOCK_BYTES,
		));
		cache.restore().await;
		assert_eq!(cache.snapshot().l2_blocks, 1, "ledger rebuilt from disk");

		let inner = Arc::new(CountingProvider {
			data: data.clone(),
			reads: reads.clone(),
			modified: Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000)),
		});
		let provider = CachedProvider::new(inner, cache);
		let got = provider.read_range(&t, 0..4096).await.unwrap();
		assert_eq!(&got[..], &data[0..4096]);
		assert_eq!(
			reads.load(std::sync::atomic::Ordering::SeqCst),
			0,
			"answered from the disk tier without touching the provider"
		);
	}

	#[tokio::test]
	async fn deleting_the_cache_directory_mid_session_is_survivable() {
		let dir = tempfile::tempdir().unwrap();
		let data = payload(2 * BLOCK_BYTES as usize);
		let (_reads, cache, provider) = harness(&dir, data.clone());
		let t = target();

		provider.read_range(&t, 0..4096).await.unwrap();
		std::fs::remove_dir_all(dir.path()).unwrap();
		// L1 still answers; the point is that L2 lookups do not fault.
		cache.clear(false).await;

		let got = provider.read_range(&t, 0..4096).await.unwrap();
		assert_eq!(&got[..], &data[0..4096]);
	}

	#[tokio::test]
	async fn eviction_holds_the_ceiling() {
		let dir = tempfile::tempdir().unwrap();
		// Room for four blocks on disk, against a file of ten.
		let cache = Arc::new(BlockCache::new(
			SourceDirs::new(dir.path().to_path_buf()).unwrap(),
			4 * BLOCK_BYTES,
		));
		let data = payload(10 * BLOCK_BYTES as usize);
		let inner = Arc::new(CountingProvider {
			data,
			reads: Arc::new(AtomicUsize::new(0)),
			modified: Some(SystemTime::UNIX_EPOCH),
		});
		let provider = CachedProvider::new(inner, cache.clone());
		let t = target();

		for block in 0..10u64 {
			provider
				.read_range(&t, block * BLOCK_BYTES..block * BLOCK_BYTES + 128)
				.await
				.unwrap();
		}

		let snap = cache.snapshot();
		assert!(snap.l2_bytes <= snap.max_bytes, "disk tier over ceiling");
		assert!(snap.evicted_blocks > 0, "nothing was evicted");
	}
}
