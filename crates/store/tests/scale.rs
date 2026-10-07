//! Cold fan-out at scale: many stores, no arenas, one search each.
//!
//! Ignored by default; run it by hand to measure the read path's floor:
//!
//! ```sh
//! SD_SCALE_STORES=100 SD_SCALE_RECORDS=10000 \
//!   cargo test -p sd-store --test scale -- --ignored --nocapture
//! ```
//!
//! The reliability plan's R6 sizes its query budget from these numbers; the
//! scan is the documented baseline a candidate title index gets benchmarked
//! against.

use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
use sd_store::{filesystem_schema, read, SourceManager};
use std::time::Instant;

fn scale(name: &str, default: usize) -> usize {
	std::env::var(name)
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(default)
}

/// Write `stores` stores of `records` records each, one match per fifty.
async fn build_stores(manager: &SourceManager, stores: usize, records: usize) {
	for store in 0..stores {
		let id = format!("store-{store}");
		manager
			.create(&id, &filesystem_schema())
			.await
			.expect("create");
		let db = manager.open(&id).await.expect("open");
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

		let mut writes = Vec::with_capacity(records);
		for record in 0..records {
			// A sprinkle of matches per store; the rest is noise the scan
			// has to fold through.
			let name = if record % 50 == 0 {
				format!("keepsake-{record}.mov")
			} else {
				format!("filler-{record}.dat")
			};
			let observation = Observation {
				external_id: name.clone(),
				kind: FileKind::File,
				name,
				size: 1_000 + record as i64,
				mtime: 1_700_000_000_000,
				created: None,
				accessed: None,
				inode: None,
				mode: Some(0o644),
				uid: None,
				gid: None,
				link_target: None,
				extension: Some("dat".to_string()),
				is_hidden: false,
				identity: None,
			};
			let resolution = ledger.resolve(&observation);
			writes.push(FileWrite {
				resolution,
				parent_uuid: None,
				observation,
			});
		}
		db.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");
		db.pool().close().await;
	}
}

struct FanOut {
	elapsed: std::time::Duration,
	slowest_ms: u128,
	open_ms_total: u128,
	total_matches: u64,
}

/// Open every store read-only, cold, and search it once.
async fn fan_out(manager: &SourceManager, stores: usize, records: usize) -> FanOut {
	let expected_per_store = records.div_ceil(50) as u64;
	let search_started = Instant::now();
	let mut slowest_ms = 0u128;
	let mut open_ms_total = 0u128;
	let mut total_matches = 0u64;
	for store in 0..stores {
		let opened = Instant::now();
		let db = manager
			.open_read_only(&format!("store-{store}"))
			.await
			.expect("read-only open");
		open_ms_total += opened.elapsed().as_millis();

		let scanned = Instant::now();
		let hits = read::search_titles(db.pool(), "keepsake", 100_000)
			.await
			.expect("search");
		slowest_ms = slowest_ms.max(scanned.elapsed().as_millis());
		assert_eq!(hits.total, expected_per_store);
		assert!(!hits.truncated);
		total_matches += hits.total;
		db.pool().close().await;
	}
	FanOut {
		elapsed: search_started.elapsed(),
		slowest_ms,
		open_ms_total,
		total_matches,
	}
}

#[tokio::test]
#[ignore = "measurement, not regression; run with --ignored --nocapture"]
async fn a_cold_fan_out_across_many_stores() {
	let stores = scale("SD_SCALE_STORES", 10);
	let records = scale("SD_SCALE_RECORDS", 1_000);

	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().to_path_buf());

	let build_started = Instant::now();
	build_stores(&manager, stores, records).await;
	let build = build_started.elapsed();
	let fan_out = fan_out(&manager, stores, records).await;

	println!(
		"scale: {stores} stores x {records} records ({} total rows)",
		stores * records
	);
	println!("  build: {:.1}s", build.as_secs_f64());
	println!(
		"  fan-out search: {:.0}ms total, slowest store scan {}ms, opens {}ms, {} matches",
		fan_out.elapsed.as_secs_f64() * 1000.0,
		fan_out.slowest_ms,
		fan_out.open_ms_total,
		fan_out.total_matches
	);
}

/// R8 "Cold search across 100 stores", the regression half of the
/// measurement above: a hundred cold stores answer a fan-out through
/// read-only opens alone, every total exact, and no store is written to. The size is small enough for CI; the budget is generous enough
/// that only a change of algorithm trips it.
#[tokio::test]
async fn a_hundred_cold_stores_answer_without_writers() {
	let stores = 100;
	let records = 200;
	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().to_path_buf());
	build_stores(&manager, stores, records).await;

	let layout = |store: usize| -> Vec<(String, u64)> {
		let mut files: Vec<(String, u64)> =
			std::fs::read_dir(dir.path().join(format!("store-{store}")))
				.expect("store dir")
				.filter_map(|entry| entry.ok())
				.map(|entry| {
					(
						entry.file_name().to_string_lossy().into_owned(),
						entry.metadata().map(|m| m.len()).unwrap_or(0),
					)
				})
				.collect();
		files.sort();
		files
	};
	let before: Vec<Vec<(String, u64)>> = (0..stores).map(layout).collect();

	let fan_out = fan_out(&manager, stores, records).await;
	assert_eq!(
		fan_out.total_matches,
		(stores * records.div_ceil(50)) as u64
	);
	assert!(
		fan_out.elapsed < std::time::Duration::from_secs(30),
		"a cold fan-out over 100 stores took {:?}",
		fan_out.elapsed
	);

	// Reads write nothing: every store's database is the size it was. SQLite
	// itself may leave an empty WAL and its shared-memory index beside a
	// database opened in WAL mode, which is not a write of ours.
	for store in 0..stores {
		let ours = |files: &[(String, u64)]| -> Vec<(String, u64)> {
			files
				.iter()
				.filter(|(name, _)| !name.ends_with("-wal") && !name.ends_with("-shm"))
				.cloned()
				.collect()
		};
		assert_eq!(
			ours(&layout(store)),
			ours(&before[store]),
			"store-{store} changed on disk under read-only reads"
		);
	}
}
