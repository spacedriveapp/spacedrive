//! Task-local flag identifying code running inside a backfill apply loop.
//!
//! A few per-record hooks (closure table rebuild, resource event emission)
//! are redundant during backfill because the coordinator does bulk work at
//! the end. Models check this flag to skip that per-record work.

tokio::task_local! {
	static IN_BACKFILL: ();
}

/// Run `fut` with the in-backfill flag set. Nested scopes are allowed.
pub async fn in_backfill<F, T>(fut: F) -> T
where
	F: std::future::Future<Output = T>,
{
	if is_in_backfill() {
		fut.await
	} else {
		IN_BACKFILL.scope((), fut).await
	}
}

/// True when the current task is inside an `in_backfill` scope.
pub fn is_in_backfill() -> bool {
	IN_BACKFILL.try_with(|_| ()).is_ok()
}
