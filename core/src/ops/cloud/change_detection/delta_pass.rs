//! High-level orchestrator that runs one delta pass end-to-end.
//!
//! Glue between a [`ChangeDetector`] implementation (OneDrive today, future
//! providers tomorrow) and a [`CloudSyncStateRepository`]. The indexer calls
//! [`run_cloud_delta_pass`] once per cloud volume per scan. The helper owns
//! the page-walking loop, token checkpointing, backoff bookkeeping, and
//! failure classification so that the indexing phase does not have to reason
//! about HTTP error variants directly.
//!
//! ## Outcomes
//! [`DeltaOutcome`] tells the caller what happened. The indexer uses it to
//! decide whether to run a full rescan, skip the pass, or proceed normally.
//! Every outcome includes enough context for structured logging — raw
//! `tracing` emission stays inside this module so call sites do not have to
//! remember the right field names.
//!
//! ## Budget and safety
//! The outer page loop is bounded by [`MAX_PAGES_PER_PASS`] as a hard safety
//! cut-off: if a provider ever loops (misbehaving mock, broken token) we stop
//! after ~1 M entries rather than spin forever. Each successful page persists
//! its `next_token`, so interrupting the loop at any point is resumable.

use super::types::{ChangeDetectionError, ChangeDetector, ChangeEntry, ChangeToken};
use super::CloudSyncStateRepository;
use std::sync::Arc;

/// Hard cap on the number of pages we drain in a single pass.
///
/// One Graph page is up to ~200 items; a million-entry drive fits in roughly
/// 5000 pages. The limit is defensive — a well-behaved provider returns a
/// `deltaLink` long before this — and exists so a buggy token never wedges
/// the indexer.
pub const MAX_PAGES_PER_PASS: usize = 50_000;

/// Classifier for the outcome of a single delta pass.
///
/// Variants are deliberately coarse so the indexer can route on them with a
/// `match` without inspecting nested data. `entries` on
/// `Incremental` carries the aggregated change set so higher layers can
/// decide whether to trigger a re-index, hash, or tag migration per entry.
#[derive(Debug)]
pub enum DeltaOutcome {
	/// The volume had no prior token, so we recorded a baseline and did NOT
	/// fetch per-entry changes. The indexer still runs its normal full scan
	/// in this pass; the baseline is used on subsequent passes.
	SeededBaseline,

	/// A delta pass completed and returned these changes. The vector may be
	/// empty when the provider has nothing new since the last baseline.
	Incremental { entries: Vec<ChangeEntry> },

	/// Provider says the stored token is too old; caller must run a full
	/// rescan. The token has already been cleared on disk.
	FullResyncNeeded,

	/// Rate-limited. `retry_after_secs` reflects the provider's `Retry-After`
	/// header when present.
	RateLimited { retry_after_secs: u64 },

	/// Authentication failed. The credential is likely invalid; the scheduler
	/// (or manual re-auth UI) must recover.
	AuthError,

	/// Anything else — transport blip, parse error, malformed response. The
	/// scheduler should treat it like a transient error and back off.
	TransientError { reason: String },
}

/// Run exactly one delta pass for a volume.
///
/// Separating this from the indexer body keeps the orchestration code
/// testable against mocked repositories and detectors without spinning up a
/// real job runtime.
pub async fn run_cloud_delta_pass(
	volume_id: i32,
	detector: Arc<dyn ChangeDetector>,
	repo: Arc<dyn CloudSyncStateRepository>,
) -> DeltaOutcome {
	let provider_id = detector.provider_id();

	let state = match repo.get(volume_id).await {
		Ok(s) => s,
		Err(e) => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %e,
				"failed to load cloud_sync_state",
			);
			return DeltaOutcome::TransientError {
				reason: format!("load state: {e}"),
			};
		}
	};

	// First-ever sync for this volume — seed a baseline token and let the
	// caller run its normal full scan. Any state row that might pre-exist
	// from a prior failed pass will be overwritten by `upsert` below.
	let existing_token = state.as_ref().and_then(|s| s.change_token.clone());
	if existing_token.is_none() {
		tracing::info!(
			volume_id,
			provider = provider_id,
			"no prior change_token — seeding baseline",
		);
		let baseline = match detector.initial_token().await {
			Ok(t) => t,
			Err(e) => return classify_error(volume_id, provider_id, &repo, e).await,
		};

		// Use `upsert` to avoid a race when two concurrent passes both see
		// `None` — the second one overwrites safely.
		let mut row = super::repository::CloudSyncState::new(volume_id, provider_id.to_string());
		row.change_token = Some(baseline.0.clone());
		if let Err(e) = repo.upsert(row).await {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %e,
				"failed to persist baseline change_token",
			);
			return DeltaOutcome::TransientError {
				reason: format!("persist baseline: {e}"),
			};
		}
		if let Err(e) = repo.mark_full_sync_complete(volume_id).await {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %e,
				"failed to mark full sync complete after baseline seed",
			);
		}
		return DeltaOutcome::SeededBaseline;
	}

	// Drain delta pages. Checkpoint the next_token after every successful
	// page so an interrupted pass resumes from the last good position.
	let mut token = ChangeToken(existing_token.unwrap_or_default());
	let mut aggregated: Vec<ChangeEntry> = Vec::new();
	let mut end_token: Option<ChangeToken> = None;

	for page_num in 0..MAX_PAGES_PER_PASS {
		let page = match detector.changes_since(&token).await {
			Ok(p) => p,
			Err(e) => return classify_error(volume_id, provider_id, &repo, e).await,
		};

		aggregated.extend(page.entries);

		if let Some(delta) = page.end_token {
			end_token = Some(delta);
			break;
		}

		let Some(next) = page.next_token else {
			// Provider returned neither next nor delta — treat as transient.
			tracing::warn!(
				volume_id,
				provider = provider_id,
				page = page_num,
				"delta page missing both next_token and end_token",
			);
			return DeltaOutcome::TransientError {
				reason: "delta page missing continuation".to_string(),
			};
		};

		// Checkpoint: any future crash resumes from this point.
		if let Err(e) = repo.update_change_token(volume_id, next.as_str()).await {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %e,
				"failed to checkpoint change_token",
			);
			return DeltaOutcome::TransientError {
				reason: format!("checkpoint: {e}"),
			};
		}
		token = next;
	}

	let Some(end) = end_token else {
		// Exhausted the page budget without reaching deltaLink. Treat as
		// transient; the checkpointed token lets the next pass continue.
		tracing::warn!(
			volume_id,
			provider = provider_id,
			pages = MAX_PAGES_PER_PASS,
			"delta pass exceeded page budget without reaching end_token",
		);
		return DeltaOutcome::TransientError {
			reason: format!("exceeded {MAX_PAGES_PER_PASS} pages"),
		};
	};

	if let Err(e) = repo.update_change_token(volume_id, end.as_str()).await {
		tracing::warn!(
			volume_id,
			provider = provider_id,
			error = %e,
			"failed to persist end_token",
		);
		return DeltaOutcome::TransientError {
			reason: format!("persist end_token: {e}"),
		};
	}
	if let Err(e) = repo.mark_incremental(volume_id).await {
		tracing::warn!(
			volume_id,
			provider = provider_id,
			error = %e,
			"failed to mark incremental sync complete",
		);
	}

	tracing::info!(
		volume_id,
		provider = provider_id,
		changes = aggregated.len(),
		"cloud delta pass complete",
	);

	DeltaOutcome::Incremental {
		entries: aggregated,
	}
}

/// Map a [`ChangeDetectionError`] to the appropriate [`DeltaOutcome`] and
/// update the failure counter.
///
/// `Invalidated` is special-cased: we clear the stored token so the next pass
/// sees `None` and reseeds. Every other error only bumps
/// `consecutive_failures`; the scheduler decides what the caller should do.
async fn classify_error(
	volume_id: i32,
	provider_id: &'static str,
	repo: &Arc<dyn CloudSyncStateRepository>,
	err: ChangeDetectionError,
) -> DeltaOutcome {
	// Best-effort failure counter bump; do not let a failure to bump the
	// counter mask the original error.
	if let Err(e) = repo.increment_failures(volume_id).await {
		tracing::warn!(
			volume_id,
			provider = provider_id,
			error = %e,
			"failed to increment cloud_sync_state.consecutive_failures",
		);
	}

	match err {
		ChangeDetectionError::Invalidated => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				"cloud delta token invalidated — clearing for next pass",
			);
			// Clearing the token is what triggers a reseed on the next pass.
			// Using `upsert` with a `None` change_token would drop unrelated
			// fields, so read-modify-write via the repo's token setter is
			// safer — but the repository exposes only a non-null setter.
			// Instead, reseed with a fresh baseline by deleting the token
			// column: we emulate that by upserting a new row that preserves
			// provider but clears the token and timestamps.
			if let Ok(Some(mut existing)) = repo.get(volume_id).await {
				existing.change_token = None;
				existing.last_full_sync_at = None;
				if let Err(e) = repo.upsert(existing).await {
					tracing::warn!(
						volume_id,
						provider = provider_id,
						error = %e,
						"failed to clear change_token after invalidation",
					);
				}
			}
			DeltaOutcome::FullResyncNeeded
		}
		ChangeDetectionError::RateLimited { retry_after_secs } => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				retry_after_secs,
				"cloud delta pass rate limited",
			);
			DeltaOutcome::RateLimited { retry_after_secs }
		}
		ChangeDetectionError::Auth(msg) => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %msg,
				"cloud delta pass auth error — credential may need refresh",
			);
			DeltaOutcome::AuthError
		}
		ChangeDetectionError::Transport(e) => {
			tracing::debug!(
				volume_id,
				provider = provider_id,
				error = %e,
				"transport error during cloud delta pass",
			);
			DeltaOutcome::TransientError {
				reason: format!("transport: {e}"),
			}
		}
		ChangeDetectionError::Parse(e) => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %e,
				"failed to parse cloud delta response",
			);
			DeltaOutcome::TransientError {
				reason: format!("parse: {e}"),
			}
		}
		ChangeDetectionError::Other(msg) => {
			tracing::warn!(
				volume_id,
				provider = provider_id,
				error = %msg,
				"unexpected error during cloud delta pass",
			);
			DeltaOutcome::TransientError { reason: msg }
		}
	}
}

/// Backoff helper separated from [`run_cloud_delta_pass`] so the indexer can
/// unit-test it without mocking the whole detector.
///
/// The plan specifies `min(retry_after_secs, 2^consecutive_failures)` with a
/// 3600s cap. Returning `u64` lets callers hand the value directly to
/// `tokio::time::sleep`.
pub fn compute_backoff_secs(retry_after_secs: Option<u64>, consecutive_failures: i32) -> u64 {
	// Exponential base-2 backoff capped at 2^12 = 4096s, which already
	// exceeds the 3600s ceiling applied below. Using plain `<<` with the
	// clamp keeps the math intentional and avoids an unstable API.
	let exp = consecutive_failures.clamp(0, 12) as u32;
	let exponential: u64 = 1u64 << exp;
	// When the provider gave us a specific retry delay, the plan asks us to
	// take the minimum with the exponential backoff so we never keep hammering
	// a quota that is actively denying us.
	let combined = match retry_after_secs {
		Some(r) => r.min(exponential),
		None => exponential,
	};
	combined.min(3600)
}

#[cfg(test)]
mod tests {
	use super::super::types::{ChangeDetectionError, ChangesPage};
	use super::*;
	use crate::ops::cloud::change_detection::{CloudSyncState, CloudSyncStateError};
	use async_trait::async_trait;
	use std::sync::Mutex;

	/// In-memory mock repository. Not shared with the SeaORM tests because
	/// using the real DB here would blow the unit-test budget without adding
	/// coverage beyond what `repository::tests` already provides.
	#[derive(Default)]
	struct MockRepo {
		state: Mutex<Option<CloudSyncState>>,
		token_updates: Mutex<Vec<String>>,
		full_sync_marks: Mutex<u32>,
		incremental_marks: Mutex<u32>,
		failure_bumps: Mutex<u32>,
		failure_resets: Mutex<u32>,
	}

	impl MockRepo {
		fn with_state(state: Option<CloudSyncState>) -> Self {
			Self {
				state: Mutex::new(state),
				..Default::default()
			}
		}

		fn token_updates(&self) -> Vec<String> {
			self.token_updates.lock().unwrap().clone()
		}
		fn full_sync_marks(&self) -> u32 {
			*self.full_sync_marks.lock().unwrap()
		}
		fn incremental_marks(&self) -> u32 {
			*self.incremental_marks.lock().unwrap()
		}
		fn failure_bumps(&self) -> u32 {
			*self.failure_bumps.lock().unwrap()
		}
	}

	#[async_trait]
	impl CloudSyncStateRepository for MockRepo {
		async fn get(&self, _vid: i32) -> Result<Option<CloudSyncState>, CloudSyncStateError> {
			Ok(self.state.lock().unwrap().clone())
		}
		async fn upsert(&self, state: CloudSyncState) -> Result<(), CloudSyncStateError> {
			*self.state.lock().unwrap() = Some(state);
			Ok(())
		}
		async fn update_change_token(
			&self,
			_vid: i32,
			token: &str,
		) -> Result<(), CloudSyncStateError> {
			self.token_updates.lock().unwrap().push(token.to_string());
			if let Some(ref mut s) = *self.state.lock().unwrap() {
				s.change_token = Some(token.to_string());
			}
			Ok(())
		}
		async fn mark_full_sync_complete(&self, _vid: i32) -> Result<(), CloudSyncStateError> {
			*self.full_sync_marks.lock().unwrap() += 1;
			Ok(())
		}
		async fn mark_incremental(&self, _vid: i32) -> Result<(), CloudSyncStateError> {
			*self.incremental_marks.lock().unwrap() += 1;
			Ok(())
		}
		async fn increment_failures(&self, _vid: i32) -> Result<i32, CloudSyncStateError> {
			*self.failure_bumps.lock().unwrap() += 1;
			Ok(1)
		}
		async fn reset_failures(&self, _vid: i32) -> Result<(), CloudSyncStateError> {
			*self.failure_resets.lock().unwrap() += 1;
			Ok(())
		}
	}

	/// Mock detector whose behavior is programmed per-call. Avoids spinning up
	/// a `wiremock::MockServer` at the unit-test tier.
	struct MockDetector {
		provider: &'static str,
		initial: Mutex<Vec<Result<ChangeToken, ChangeDetectionError>>>,
		deltas: Mutex<Vec<Result<ChangesPage, ChangeDetectionError>>>,
	}

	impl MockDetector {
		fn new_seed(token: &str) -> Self {
			Self {
				provider: "mock",
				initial: Mutex::new(vec![Ok(ChangeToken(token.to_string()))]),
				deltas: Mutex::new(Vec::new()),
			}
		}

		fn new_delta_pages(pages: Vec<Result<ChangesPage, ChangeDetectionError>>) -> Self {
			Self {
				provider: "mock",
				initial: Mutex::new(Vec::new()),
				deltas: Mutex::new(pages),
			}
		}
	}

	#[async_trait]
	impl ChangeDetector for MockDetector {
		fn provider_id(&self) -> &'static str {
			self.provider
		}

		async fn initial_token(&self) -> Result<ChangeToken, ChangeDetectionError> {
			self.initial.lock().unwrap().remove(0)
		}

		async fn changes_since(
			&self,
			_token: &ChangeToken,
		) -> Result<ChangesPage, ChangeDetectionError> {
			self.deltas.lock().unwrap().remove(0)
		}
	}

	#[tokio::test]
	async fn test_no_prior_token_seeds_baseline() {
		let repo = Arc::new(MockRepo::with_state(None));
		let detector = Arc::new(MockDetector::new_seed("baseline-1"));

		let outcome = run_cloud_delta_pass(
			1,
			detector,
			repo.clone() as Arc<dyn CloudSyncStateRepository>,
		)
		.await;

		assert!(matches!(outcome, DeltaOutcome::SeededBaseline));
		let state = repo.state.lock().unwrap().clone().unwrap();
		assert_eq!(state.change_token.as_deref(), Some("baseline-1"));
		assert_eq!(repo.full_sync_marks(), 1);
	}

	#[tokio::test]
	async fn test_delta_drains_pages_and_marks_incremental() {
		let mut seed = CloudSyncState::new(1, "mock");
		seed.change_token = Some("start".into());
		let repo = Arc::new(MockRepo::with_state(Some(seed)));

		let entry = |id: &str| ChangeEntry {
			provider_file_id: id.into(),
			path: format!("/{id}"),
			kind: super::super::types::ChangeKind::Modified,
			etag: None,
			last_modified: None,
			size: None,
			is_folder: false,
		};

		let pages = vec![
			Ok(ChangesPage {
				entries: vec![entry("a"), entry("b")],
				next_token: Some(ChangeToken("page-2".into())),
				end_token: None,
			}),
			Ok(ChangesPage {
				entries: vec![entry("c")],
				next_token: None,
				end_token: Some(ChangeToken("final".into())),
			}),
		];
		let detector = Arc::new(MockDetector::new_delta_pages(pages));

		let outcome = run_cloud_delta_pass(
			1,
			detector,
			repo.clone() as Arc<dyn CloudSyncStateRepository>,
		)
		.await;

		match outcome {
			DeltaOutcome::Incremental { entries } => assert_eq!(entries.len(), 3),
			other => panic!("expected Incremental, got {other:?}"),
		}
		// First page checkpoints "page-2", end persists "final".
		let tokens = repo.token_updates();
		assert_eq!(tokens, vec!["page-2".to_string(), "final".to_string()]);
		assert_eq!(repo.incremental_marks(), 1);
	}

	#[tokio::test]
	async fn test_invalidated_clears_token_and_requests_full_resync() {
		let mut seed = CloudSyncState::new(1, "mock");
		seed.change_token = Some("stale".into());
		let repo = Arc::new(MockRepo::with_state(Some(seed)));

		let detector = Arc::new(MockDetector::new_delta_pages(vec![Err(
			ChangeDetectionError::Invalidated,
		)]));

		let outcome = run_cloud_delta_pass(
			1,
			detector,
			repo.clone() as Arc<dyn CloudSyncStateRepository>,
		)
		.await;

		assert!(matches!(outcome, DeltaOutcome::FullResyncNeeded));
		assert_eq!(repo.failure_bumps(), 1);
		let state = repo.state.lock().unwrap().clone().unwrap();
		assert!(
			state.change_token.is_none(),
			"Invalidated must clear the token so the next pass reseeds"
		);
	}

	#[tokio::test]
	async fn test_rate_limited_surfaces_retry_after() {
		let mut seed = CloudSyncState::new(1, "mock");
		seed.change_token = Some("cur".into());
		let repo = Arc::new(MockRepo::with_state(Some(seed)));

		let detector = Arc::new(MockDetector::new_delta_pages(vec![Err(
			ChangeDetectionError::RateLimited {
				retry_after_secs: 42,
			},
		)]));

		let outcome = run_cloud_delta_pass(
			1,
			detector,
			repo.clone() as Arc<dyn CloudSyncStateRepository>,
		)
		.await;

		match outcome {
			DeltaOutcome::RateLimited { retry_after_secs } => {
				assert_eq!(retry_after_secs, 42);
			}
			other => panic!("expected RateLimited, got {other:?}"),
		}
		assert_eq!(repo.failure_bumps(), 1);
	}

	#[tokio::test]
	async fn test_auth_error_maps_to_auth_outcome() {
		let mut seed = CloudSyncState::new(1, "mock");
		seed.change_token = Some("cur".into());
		let repo = Arc::new(MockRepo::with_state(Some(seed)));

		let detector = Arc::new(MockDetector::new_delta_pages(vec![Err(
			ChangeDetectionError::Auth("invalid_token".into()),
		)]));

		let outcome = run_cloud_delta_pass(
			1,
			detector,
			repo.clone() as Arc<dyn CloudSyncStateRepository>,
		)
		.await;

		assert!(matches!(outcome, DeltaOutcome::AuthError));
		assert_eq!(repo.failure_bumps(), 1);
	}

	#[tokio::test]
	async fn test_backoff_respects_retry_after_and_cap() {
		// When Retry-After is shorter than exponential, honor it (provider
		// knows better).
		assert_eq!(compute_backoff_secs(Some(17), 10), 17);
		// No Retry-After → exponential.
		assert_eq!(compute_backoff_secs(None, 3), 8);
		// 3600s cap holds even at huge failure counts.
		assert_eq!(compute_backoff_secs(None, 100), 3600);
		// Zero failures: 1s floor (2^0).
		assert_eq!(compute_backoff_secs(None, 0), 1);
	}
}
