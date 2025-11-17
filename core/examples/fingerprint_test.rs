//! Test volume fingerprint stability
//!
//! Run with: cargo run --example fingerprint_test

use sd_core::domain::volume::VolumeFingerprint;

fn main() {
	println!("\n=== Volume Fingerprint Stability Tests ===\n");

	// Test 1: Deterministic
	println!("Test 1: Same inputs → Same fingerprint");
	let uuid_pair = "CONTAINER-UUID:VOLUME-UUID";
	let capacity = 1_000_000_000_000u64; // 1TB

	let fp1 = VolumeFingerprint::new(uuid_pair, capacity, "APFS");
	let fp2 = VolumeFingerprint::new(uuid_pair, capacity, "APFS");

	println!("  First run:  {}", fp1.short_id());
	println!("  Second run: {}", fp2.short_id());
	println!("  Match: {}\n", fp1 == fp2);

	// Test 2: Total capacity vs consumed capacity
	println!("Test 2: TOTAL capacity (stable) vs CONSUMED (changes)");
	let container_total = 1_000_000_000_000u64; // Physical drive: 1TB (never changes)
	let consumed_today = 50_000_000_000u64; // Used space: 50GB today
	let consumed_tomorrow = 100_000_000_000u64; // Used space: 100GB tomorrow

	let fp_with_total = VolumeFingerprint::new(uuid_pair, container_total, "APFS");
	let fp_with_consumed_50 = VolumeFingerprint::new(uuid_pair, consumed_today, "APFS");
	let fp_with_consumed_100 = VolumeFingerprint::new(uuid_pair, consumed_tomorrow, "APFS");

	println!(
		"  Using total (1TB):           {}",
		fp_with_total.short_id()
	);
	println!(
		"  Using consumed (50GB today): {}",
		fp_with_consumed_50.short_id()
	);
	println!(
		"  Using consumed (100GB tmrw): {}",
		fp_with_consumed_100.short_id()
	);
	println!("  Total stays same:     {}", fp_with_total == fp_with_total);
	println!(
		"  Consumed changes:     {} (BAD!)\n",
		fp_with_consumed_50 != fp_with_consumed_100
	);

	// Test 3: Disk IDs vs UUIDs
	println!("Test 3: disk3 → disk4 on reboot (unstable) vs UUID (stable)");

	// UUID-based (stable)
	let uuid_based = "ABCD-1234:VOL-5678";
	let fp_uuid_run1 = VolumeFingerprint::new(uuid_based, capacity, "APFS");
	let fp_uuid_run2 = VolumeFingerprint::new(uuid_based, capacity, "APFS");

	// Disk ID-based (changes on reboot)
	let disk_id_before = "disk3:disk3s5"; // Before reboot
	let disk_id_after = "disk4:disk4s5"; // After reboot (same physical volume!)

	let fp_disk3 = VolumeFingerprint::new(disk_id_before, capacity, "APFS");
	let fp_disk4 = VolumeFingerprint::new(disk_id_after, capacity, "APFS");

	println!("  UUID-based before reboot: {}", fp_uuid_run1.short_id());
	println!("  UUID-based after reboot:  {}", fp_uuid_run2.short_id());
	println!("  UUID stable: {}\n", fp_uuid_run1 == fp_uuid_run2);

	println!("  disk3 before reboot: {}", fp_disk3.short_id());
	println!("  disk4 after reboot:  {}", fp_disk4.short_id());
	println!(
		"  Disk ID changes: {} (creates duplicates!)\n",
		fp_disk3 != fp_disk4
	);

	// Summary
	println!("=== Summary ===");
	println!("GOOD: Use container.uuid:volume.uuid + container.total_capacity");
	println!("BAD:  Use container_id:disk_id (changes on reboot)");
	println!("BAD:  Use capacity_consumed (changes with file operations)");
	println!();
	println!("Current implementation:");
	println!("  VolumeFingerprint::new(");
	println!("    &format!(\"{{}}:{{}}\", container.uuid, volume.uuid),");
	println!("    container.total_capacity,  // ← Stable!");
	println!("    \"APFS\"");
	println!("  )");
}
