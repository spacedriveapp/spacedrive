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

#[tokio::test]
#[ignore = "measurement, not regression; run with --ignored --nocapture"]
async fn a_cold_fan_out_across_many_stores() {
	let stores = scale("SD_SCALE_STORES", 10);
	let records = scale("SD_SCALE_RECORDS", 1_000);

	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().to_path_buf());

	let build_started = Instant::now();
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
	let build = build_started.elapsed();

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
	let fan_out = search_started.elapsed();

	println!(
		"scale: {stores} stores x {records} records ({} total rows)",
		stores * records
	);
	println!("  build: {:.1}s", build.as_secs_f64());
	println!(
		"  fan-out search: {:.0}ms total, slowest store scan {slowest_ms}ms, opens {open_ms_total}ms, {total_matches} matches",
		fan_out.as_secs_f64() * 1000.0
	);
}
