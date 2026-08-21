use std::{
	fs::OpenOptions,
	io::{Seek, SeekFrom, Write},
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	thread,
};

use sd_pvcache::{Error, Pvcache, PvcacheReader, TileState};
use tempfile::TempDir;
use uuid::Uuid;

const TILE_W: u32 = 64;
const TILE_H: u32 = 64;
const TILE_LEN: usize = (TILE_W * TILE_H * 4) as usize;

fn cache_path() -> (TempDir, PathBuf) {
	let dir = tempfile::tempdir().expect("tempdir");
	let path = dir.path().join("thumbs.pvcache");
	(dir, path)
}

fn tile(byte: u8) -> Vec<u8> {
	vec![byte; TILE_LEN]
}

fn assert_uniform(buf: &[u8], byte: u8, context: &str) {
	assert!(
		buf.iter().all(|b| *b == byte),
		"torn or wrong tile ({context}): expected uniform {byte:#04x}"
	);
}

#[test]
fn roundtrip_write_read() {
	let (_dir, path) = cache_path();
	let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
	assert_eq!(cache.tile_len(), TILE_LEN);

	let entries: Vec<(Uuid, u8)> = (0..8u8).map(|i| (Uuid::now_v7(), 0x10 + i)).collect();
	for (uuid, byte) in &entries {
		cache
			.write(*uuid, u64::from(*byte), &tile(*byte))
			.expect("write");
	}
	assert_eq!(cache.len(), entries.len());

	let mut buf = vec![0u8; TILE_LEN];
	for (uuid, byte) in &entries {
		let state = cache.get(*uuid, u64::from(*byte), &mut buf).expect("get");
		assert_eq!(state, TileState::Fresh);
		assert_uniform(&buf, *byte, "roundtrip");
	}

	assert_eq!(
		cache.get(Uuid::now_v7(), 0, &mut buf).expect("get"),
		TileState::Absent
	);
}

#[test]
fn reopen_persists_slot_index() {
	let (_dir, path) = cache_path();
	let entries: Vec<(Uuid, u8)> = (0..5u8).map(|i| (Uuid::now_v7(), 0xA0 + i)).collect();

	{
		let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
		for (uuid, byte) in &entries {
			cache
				.write(*uuid, u64::from(*byte), &tile(*byte))
				.expect("write");
		}
	}

	let cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("reopen");
	assert_eq!(cache.len(), entries.len());
	let mut buf = vec![0u8; TILE_LEN];
	for (uuid, byte) in &entries {
		assert_eq!(
			cache.get(*uuid, u64::from(*byte), &mut buf).expect("get"),
			TileState::Fresh
		);
		assert_uniform(&buf, *byte, "reopen");
	}
}

#[test]
fn growth_across_doubling_preserves_earlier_slots() {
	let (_dir, path) = cache_path();
	let mut cache = Pvcache::open_or_create_with_capacity(&path, TILE_W, TILE_H, 2).expect("open");
	assert_eq!(cache.capacity(), 2);

	let entries: Vec<(Uuid, u8)> = (0..33u8).map(|i| (Uuid::now_v7(), i)).collect();
	for (uuid, byte) in &entries {
		cache
			.write(*uuid, u64::from(*byte), &tile(*byte))
			.expect("write");
	}
	assert!(cache.capacity() >= 33, "capacity doubled past demand");
	assert_eq!(cache.len(), 33);

	let mut buf = vec![0u8; TILE_LEN];
	for (uuid, byte) in &entries {
		assert_eq!(
			cache.get(*uuid, u64::from(*byte), &mut buf).expect("get"),
			TileState::Fresh
		);
		assert_uniform(&buf, *byte, "growth");
	}

	// The grown file reopens with everything intact.
	drop(cache);
	let cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("reopen");
	assert_eq!(cache.len(), 33);
	for (uuid, byte) in &entries {
		assert_eq!(
			cache.get(*uuid, u64::from(*byte), &mut buf).expect("get"),
			TileState::Fresh
		);
		assert_uniform(&buf, *byte, "growth reopen");
	}
}

#[test]
fn version_staleness_detection() {
	let (_dir, path) = cache_path();
	let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
	let uuid = Uuid::now_v7();

	cache.write(uuid, 10, &tile(0x11)).expect("write v10");
	assert_eq!(cache.lookup(uuid, 10), TileState::Fresh);
	assert_eq!(cache.lookup(uuid, 11), TileState::Stale { version: 10 });

	// A stale hit still hands back the old pixels for display while rebaking.
	let mut buf = vec![0u8; TILE_LEN];
	assert_eq!(
		cache.get(uuid, 11, &mut buf).expect("get"),
		TileState::Stale { version: 10 }
	);
	assert_uniform(&buf, 0x11, "stale pixels");

	// Rebaking in place flips the slot back to fresh at the new version.
	cache.write(uuid, 11, &tile(0x22)).expect("rebake v11");
	assert_eq!(
		cache.get(uuid, 11, &mut buf).expect("get"),
		TileState::Fresh
	);
	assert_uniform(&buf, 0x22, "rebaked pixels");
	assert_eq!(cache.lookup(uuid, 10), TileState::Stale { version: 11 });
}

#[test]
fn reader_discovers_entries_written_after_open() {
	let (_dir, path) = cache_path();
	let mut cache = Pvcache::open_or_create_with_capacity(&path, TILE_W, TILE_H, 1).expect("open");
	let first = Uuid::now_v7();
	cache.write(first, 1, &tile(0x01)).expect("write");

	let mut reader = PvcacheReader::open(&path).expect("reader open");
	let mut buf = vec![0u8; TILE_LEN];
	assert_eq!(
		reader.get(first, 1, &mut buf).expect("get"),
		TileState::Fresh
	);

	// Entries written after the reader opened, across several capacity
	// doublings, are discovered via the shared header.
	let later: Vec<(Uuid, u8)> = (0..10u8).map(|i| (Uuid::now_v7(), 0x30 + i)).collect();
	for (uuid, byte) in &later {
		cache
			.write(*uuid, u64::from(*byte), &tile(*byte))
			.expect("write");
	}
	assert!(cache.capacity() >= 11);

	for (uuid, byte) in &later {
		assert_eq!(
			reader.get(*uuid, u64::from(*byte), &mut buf).expect("get"),
			TileState::Fresh
		);
		assert_uniform(&buf, *byte, "reader discovery");
	}
	assert_eq!(
		reader.get(first, 1, &mut buf).expect("get"),
		TileState::Fresh
	);
	assert_uniform(&buf, 0x01, "reader old slot after remap");
}

#[test]
fn concurrent_overwrite_has_no_torn_reads() {
	let (_dir, path) = cache_path();
	let target = Uuid::now_v7();
	let extras: Arc<Vec<Uuid>> = Arc::new((0..256).map(|_| Uuid::now_v7()).collect());

	let mut cache = Pvcache::open_or_create_with_capacity(&path, TILE_W, TILE_H, 4).expect("open");
	cache.write(target, 0, &tile(0)).expect("seed");

	let done = Arc::new(AtomicBool::new(false));
	const ITERATIONS: u64 = 3000;
	const EXTRA_VERSION: u64 = 7;

	let writer = {
		let done = Arc::clone(&done);
		let extras = Arc::clone(&extras);
		thread::spawn(move || {
			for version in 1..=ITERATIONS {
				let byte = (version % 251) as u8;
				cache
					.write(target, version, &tile(byte))
					.expect("overwrite");
				// Interleave inserts so growth (and reader remaps) happen
				// while the target slot is under sustained rewrite.
				if version % 16 == 0 {
					let extra = extras[(version / 16 - 1) as usize % extras.len()];
					cache
						.write(extra, EXTRA_VERSION, &tile(0xEE))
						.expect("insert");
				}
			}
			done.store(true, Ordering::Release);
			cache
		})
	};

	let reader = {
		let done = Arc::clone(&done);
		let extras = Arc::clone(&extras);
		let path = path.clone();
		thread::spawn(move || {
			let mut reader = PvcacheReader::open(&path).expect("reader open");
			let mut buf = vec![0u8; TILE_LEN];
			let mut consistent_reads = 0u64;
			let mut i = 0usize;
			while !done.load(Ordering::Acquire) {
				// The expected version is deliberately behind the writer, so
				// hits report Stale with the version the pixels were baked
				// at — which must match the pixel pattern exactly.
				match reader.get(target, 0, &mut buf).expect("read") {
					TileState::Fresh => {
						assert_uniform(&buf, 0, "target v0");
						consistent_reads += 1;
					}
					TileState::Stale { version } => {
						assert_uniform(&buf, (version % 251) as u8, "target under overwrite");
						consistent_reads += 1;
					}
					TileState::Absent => {}
				}
				let extra = extras[i % extras.len()];
				i += 1;
				match reader.get(extra, EXTRA_VERSION, &mut buf).expect("read") {
					TileState::Fresh => assert_uniform(&buf, 0xEE, "extra insert"),
					TileState::Stale { version } => {
						panic!("extra entry reported unexpected version {version}")
					}
					TileState::Absent => {}
				}
			}
			consistent_reads
		})
	};

	let cache = writer.join().expect("writer thread");
	let consistent_reads = reader.join().expect("reader thread");
	assert!(
		consistent_reads > 0,
		"reader never observed a consistent tile"
	);
	assert!(cache.capacity() > 4, "growth occurred during the run");

	// Settled state: a fresh reader sees the final version everywhere.
	let mut reader = PvcacheReader::open(&path).expect("reader reopen");
	let mut buf = vec![0u8; TILE_LEN];
	assert_eq!(
		reader.get(target, ITERATIONS, &mut buf).expect("read"),
		TileState::Fresh
	);
	assert_uniform(&buf, (ITERATIONS % 251) as u8, "final target");
	for extra in extras.iter().take((ITERATIONS / 16) as usize) {
		assert_eq!(
			reader.get(*extra, EXTRA_VERSION, &mut buf).expect("read"),
			TileState::Fresh
		);
		assert_uniform(&buf, 0xEE, "final extra");
	}
}

#[test]
fn truncated_file_is_treated_as_absent() {
	let (_dir, path) = cache_path();
	{
		let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
		cache.write(Uuid::now_v7(), 1, &tile(0x55)).expect("write");
	}
	OpenOptions::new()
		.write(true)
		.open(&path)
		.expect("open raw")
		.set_len(100)
		.expect("truncate");

	assert!(matches!(
		PvcacheReader::open(&path),
		Err(Error::Incompatible)
	));

	let cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("recreate");
	assert!(cache.is_empty());
}

#[test]
fn bad_magic_is_treated_as_absent() {
	let (_dir, path) = cache_path();
	let uuid = Uuid::now_v7();
	{
		let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
		cache.write(uuid, 1, &tile(0x55)).expect("write");
	}
	let mut file = OpenOptions::new()
		.write(true)
		.open(&path)
		.expect("open raw");
	file.seek(SeekFrom::Start(0)).expect("seek");
	file.write_all(b"NOTCACHE").expect("clobber magic");
	drop(file);

	assert!(matches!(
		PvcacheReader::open(&path),
		Err(Error::Incompatible)
	));

	let cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("recreate");
	assert!(cache.is_empty());
	assert!(!cache.contains(uuid));
}

#[test]
fn unsupported_format_version_is_refused() {
	let (_dir, path) = cache_path();
	{
		Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
	}
	// The format version field sits directly after the 8-byte magic.
	let mut file = OpenOptions::new()
		.write(true)
		.open(&path)
		.expect("open raw");
	file.seek(SeekFrom::Start(8)).expect("seek");
	file.write_all(&999u32.to_le_bytes())
		.expect("clobber version");
	drop(file);

	assert!(matches!(
		PvcacheReader::open(&path),
		Err(Error::Incompatible)
	));

	let cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("recreate");
	assert!(cache.is_empty());
}

#[test]
fn missing_file_reader_fails_cleanly() {
	let (_dir, path) = cache_path();
	assert!(matches!(PvcacheReader::open(&path), Err(Error::Io(_))));
}

#[test]
fn tile_geometry_change_reinitializes() {
	let (_dir, path) = cache_path();
	let uuid = Uuid::now_v7();
	{
		let mut cache = Pvcache::open_or_create(&path, 64, 64).expect("open");
		cache
			.write(uuid, 1, &vec![0x77; 64 * 64 * 4])
			.expect("write");
	}

	let cache = Pvcache::open_or_create(&path, 128, 128).expect("reopen with new dims");
	assert!(cache.is_empty());
	assert_eq!(cache.tile_len(), 128 * 128 * 4);
	assert!(!cache.contains(uuid));
}

#[test]
fn mismatched_buffer_lengths_are_rejected() {
	let (_dir, path) = cache_path();
	let mut cache = Pvcache::open_or_create(&path, TILE_W, TILE_H).expect("open");
	let uuid = Uuid::now_v7();

	assert!(matches!(
		cache.write(uuid, 1, &[0u8; 16]),
		Err(Error::TileLengthMismatch { .. })
	));

	cache.write(uuid, 1, &tile(0x01)).expect("write");
	let mut short = vec![0u8; TILE_LEN - 1];
	assert!(matches!(
		cache.get(uuid, 1, &mut short),
		Err(Error::TileLengthMismatch { .. })
	));
}
