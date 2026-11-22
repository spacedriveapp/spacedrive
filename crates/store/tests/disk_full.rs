//! # Disk full
//!
//! A full disk must cost the write that hit it, never the store.
//!
//! SQLite rolls a transaction back by itself when a write fails for lack of
//! disk space. sqlx keeps counting the transaction, so the pooled connection
//! came back to the pool believing it was inside one, and the next failure on
//! it left a real write transaction open. Every later writer then waited out
//! the busy timeout, for as long as the process lived.

use std::time::Duration;

use sd_store::{filesystem_schema, SourceManager};
use sqlx::Connection;
use tempfile::TempDir;

/// Stand in for SQLite's own rollback on `SQLITE_FULL`, which needs a full
/// volume to happen for real: the transaction ends underneath sqlx.
async fn rolled_back_underneath(pool: &sqlx::SqlitePool) {
	let mut conn = pool.acquire().await.expect("connection");
	let mut tx = conn.begin().await.expect("begin");
	sqlx::query("INSERT INTO _sync_state (key, value, updated_at) VALUES ('lost', 'x', '')")
		.execute(&mut *tx)
		.await
		.expect("write");
	sqlx::query("ROLLBACK")
		.execute(&mut *tx)
		.await
		.expect("rollback underneath");
}

/// A released connection reaches the pool from a spawned task. Wait for it,
/// so the next acquire can be handed the connection the test is about.
async fn settle(pool: &sqlx::SqlitePool) {
	for _ in 0..200 {
		if pool.num_idle() as u32 == pool.size() {
			return;
		}
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
	panic!("the pool never settled");
}

#[tokio::test]
async fn a_transaction_sqlite_ended_does_not_leave_its_connection_holding_the_store() {
	let dir = TempDir::new().unwrap();
	let manager = SourceManager::new(dir.path().to_path_buf());
	let db = manager
		.ensure("source", &filesystem_schema())
		.await
		.expect("store");

	rolled_back_underneath(db.pool()).await;
	settle(db.pool()).await;

	// No connection the pool holds is inside a transaction.
	let mut held = Vec::new();
	for _ in 0..db.pool().size() {
		held.push(db.pool().acquire().await.expect("connection"));
	}
	for conn in &held {
		assert!(
			!conn.is_in_transaction(),
			"a connection came back to the pool counting a transaction"
		);
	}

	// A failed transaction must end, or its connection holds the write lock.
	for conn in &mut held {
		let mut tx = conn.begin().await.expect("begin");
		sqlx::query("INSERT INTO _sync_state (key, value, updated_at) VALUES ('a', '1', '')")
			.execute(&mut *tx)
			.await
			.expect("write");
		sqlx::query("INSERT INTO _sync_state (key, value, updated_at) VALUES ('a', '2', '')")
			.execute(&mut *tx)
			.await
			.expect_err("duplicate key");
	}
	drop(held);
	settle(db.pool()).await;

	tokio::time::timeout(Duration::from_secs(3), db.set_cursor("after", "landed"))
		.await
		.expect("a later write waits on nobody")
		.expect("a later write lands");

	// Read through a pool of its own, so an uncommitted write cannot pass.
	let reader = SourceManager::new(dir.path().to_path_buf())
		.open_read_only("source")
		.await
		.expect("reader");
	assert_eq!(
		reader.get_cursor("after").await.unwrap().as_deref(),
		Some("landed")
	);
	assert_eq!(reader.get_cursor("a").await.unwrap(), None);
}

#[tokio::test]
async fn sqlite_full_is_recognised_as_disk_full() {
	let dir = TempDir::new().unwrap();
	let manager = SourceManager::new(dir.path().to_path_buf());
	let db = manager
		.ensure("source", &filesystem_schema())
		.await
		.expect("store");

	let mut conn = db.pool().acquire().await.expect("connection");
	let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
		.fetch_one(&mut *conn)
		.await
		.unwrap();
	sqlx::query(&format!("PRAGMA max_page_count = {pages}"))
		.execute(&mut *conn)
		.await
		.unwrap();
	let error = sqlx::query(
		"INSERT INTO _sync_state (key, value, updated_at) VALUES ('big', randomblob(1000000), '')",
	)
	.execute(&mut *conn)
	.await
	.expect_err("the page budget is spent");
	conn.close_on_drop();

	assert!(sd_store::Error::from(error).is_disk_full());
	assert!(!sd_store::Error::Other("x".into()).is_disk_full());
}
