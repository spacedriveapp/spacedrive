//! OAuth flow state machine and in-memory store.
//!
//! A `OauthFlow` represents a single in-progress authorization grant. Flows
//! live in an `OauthFlowStore` (thin wrapper around `Arc<DashMap<Uuid, Flow>>`)
//! so multiple concurrent flows (e.g. user opens two provider modals) are
//! supported without global locking. A janitor task evicts flows that exceed
//! the pending TTL (10 minutes) or linger more than 30 seconds after reaching
//! a terminal state.

use super::provider::TokenSet;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use uuid::Uuid;

/// Maximum time a pending flow may remain in the store before being evicted.
pub const PENDING_TTL: Duration = Duration::from_secs(600);

/// Grace period after a terminal state (Completed/Failed/Cancelled) before eviction.
///
/// Gives the frontend a window to poll and read the final status; without this
/// grace, a slow poll could race the janitor and see `UnknownFlow`.
pub const TERMINAL_TTL: Duration = Duration::from_secs(30);

/// Janitor cadence. Short enough that expired flows disappear promptly, long
/// enough that the wakeup cost is negligible on an idle daemon.
pub const JANITOR_INTERVAL: Duration = Duration::from_secs(60);

/// Status of a flow as observed by the UI via the poll query.
///
/// Serialized as a `serde` externally-tagged enum so TypeScript can
/// discriminate by `type` on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OauthFlowStatus {
	/// Waiting for the user to complete the browser redirect.
	Pending,
	/// Authorization succeeded; `TokenSet` and optional display name are available.
	///
	/// `tokens` is kept nested (not flattened) so the wire JSON matches the
	/// TypeScript type emitted by specta, which does not honour `#[serde(flatten)]`.
	Completed {
		tokens: TokenSet,
		#[serde(skip_serializing_if = "Option::is_none")]
		display_name: Option<String>,
	},
	/// Authorization failed; `error` is a human-readable message.
	Failed { error: String },
	/// The user (or the UI) cancelled the flow via `cloud.oauth.cancel`.
	Cancelled,
}

impl OauthFlowStatus {
	/// Returns true if the flow has reached a terminal state and should be
	/// tracked by `terminal_at` for grace-period eviction.
	pub fn is_terminal(&self) -> bool {
		!matches!(self, OauthFlowStatus::Pending)
	}
}

/// State of a single in-progress OAuth authorization flow.
///
/// Created by `CloudOauthStartAction`, consumed by the loopback callback or
/// `CloudOauthCancelAction`. Cloned cheaply — the non-`Clone` cancellation
/// watcher lives separately in the [`OauthFlowStore`].
#[derive(Debug)]
pub struct OauthFlow {
	/// Flow identifier; the UI addresses a flow by this id.
	pub id: Uuid,
	/// Provider id as registered in [`crate::ops::cloud::oauth::OauthProviderRegistry`].
	pub provider_id: String,
	/// BYO client id; retained only until the background task exchanges the code.
	pub client_id: String,
	/// BYO client secret; same lifetime as `client_id`.
	pub client_secret: String,
	/// Fully-qualified loopback redirect URI (e.g. `http://127.0.0.1:53682`).
	pub redirect_uri: String,
	/// CSRF token echoed by the authorization endpoint.
	pub state: String,
	/// PKCE verifier held server-side until code exchange (RFC 7636).
	pub pkce_verifier: String,
	/// When the flow was created, used by the janitor for the pending TTL.
	pub created_at: chrono::DateTime<chrono::Utc>,
	/// When the flow first transitioned to a terminal state; used with
	/// [`TERMINAL_TTL`] for grace-period eviction.
	pub terminal_at: Option<chrono::DateTime<chrono::Utc>>,
	/// Current status observable by the UI.
	pub status: OauthFlowStatus,
}

/// In-memory container for active OAuth flows.
///
/// Wraps `DashMap<Uuid, OauthFlow>` so concurrent starts / polls / cancels do
/// not block each other, and pairs each flow with a [`watch::Sender<bool>`]
/// used to cancel the loopback server from `CloudOauthCancelAction` without
/// racing the `accept()` future.
#[derive(Clone, Default)]
pub struct OauthFlowStore {
	flows: Arc<DashMap<Uuid, OauthFlow>>,
	cancellers: Arc<DashMap<Uuid, watch::Sender<bool>>>,
}

impl OauthFlowStore {
	/// Build a fresh, empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Insert a new flow and register its cancellation sender.
	///
	/// Overwrites any existing flow with the same id; callers must guarantee
	/// uniqueness (`Uuid::new_v4()` is sufficient in practice).
	pub fn insert(&self, flow: OauthFlow, canceller: watch::Sender<bool>) {
		let id = flow.id;
		self.flows.insert(id, flow);
		self.cancellers.insert(id, canceller);
	}

	/// Snapshot a flow's status without removing it from the store.
	///
	/// Returning a clone of the status (not the full flow) keeps secrets like
	/// `pkce_verifier` and `client_secret` off the wire.
	pub fn status(&self, id: &Uuid) -> Option<OauthFlowStatus> {
		self.flows.get(id).map(|e| e.status.clone())
	}

	/// Apply a mutation to a flow in place. Returns `true` if the flow exists.
	///
	/// Used by the callback task to transition `Pending` → `Completed`/`Failed`
	/// and by the cancel action to transition to `Cancelled`.
	pub fn mutate<F: FnOnce(&mut OauthFlow)>(&self, id: &Uuid, f: F) -> bool {
		if let Some(mut entry) = self.flows.get_mut(id) {
			f(entry.value_mut());
			true
		} else {
			false
		}
	}

	/// Read a snapshot of the client credentials and redirect URI needed by the
	/// callback task to complete the flow without cloning the whole struct.
	pub fn exchange_context(&self, id: &Uuid) -> Option<ExchangeContext> {
		self.flows.get(id).map(|e| ExchangeContext {
			provider_id: e.provider_id.clone(),
			client_id: e.client_id.clone(),
			client_secret: e.client_secret.clone(),
			redirect_uri: e.redirect_uri.clone(),
			pkce_verifier: e.pkce_verifier.clone(),
		})
	}

	/// Signal the loopback server to stop by flipping the watch value to `true`.
	///
	/// No-op if the flow is unknown. Kept separate from status mutation because
	/// the callback task may already have observed `true` and exited.
	pub fn cancel(&self, id: &Uuid) {
		if let Some(sender) = self.cancellers.get(id) {
			let _ = sender.send(true);
		}
	}

	/// Remove a flow and its canceller. Used by the janitor and on drop.
	pub fn remove(&self, id: &Uuid) {
		self.flows.remove(id);
		self.cancellers.remove(id);
	}

	/// Evict flows whose pending window elapsed or whose terminal grace period
	/// ended. Returns the number of flows dropped — surfaced by tests and
	/// available to the janitor for tracing.
	pub fn sweep(&self, now: chrono::DateTime<chrono::Utc>) -> usize {
		let pending_cutoff = now - chrono::Duration::from_std(PENDING_TTL).unwrap_or_default();
		let terminal_cutoff = now - chrono::Duration::from_std(TERMINAL_TTL).unwrap_or_default();

		let mut to_remove = Vec::new();
		for entry in self.flows.iter() {
			let flow = entry.value();
			let expired = match (&flow.status, flow.terminal_at) {
				(OauthFlowStatus::Pending, _) => flow.created_at < pending_cutoff,
				(_, Some(ts)) => ts < terminal_cutoff,
				// Terminal state without timestamp: treat as expired to be safe.
				(_, None) => true,
			};
			if expired {
				to_remove.push(flow.id);
			}
		}

		let removed = to_remove.len();
		for id in to_remove {
			self.remove(&id);
		}
		removed
	}

	/// Count of live flows. Intended for telemetry and tests.
	pub fn len(&self) -> usize {
		self.flows.len()
	}

	/// True if no flows are live.
	pub fn is_empty(&self) -> bool {
		self.flows.is_empty()
	}
}

/// Credentials extracted from a flow so the callback task can exchange the
/// authorization code without holding a DashMap guard across an `.await`.
pub struct ExchangeContext {
	pub provider_id: String,
	pub client_id: String,
	pub client_secret: String,
	pub redirect_uri: String,
	pub pkce_verifier: String,
}

/// Run the janitor loop forever. Intended to be spawned once at startup.
///
/// Ticks every [`JANITOR_INTERVAL`] and sweeps expired flows. The function
/// only ends if the runtime shuts down (returns `!`).
pub async fn run_janitor(store: OauthFlowStore) -> ! {
	let mut ticker = tokio::time::interval(JANITOR_INTERVAL);
	ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
	loop {
		ticker.tick().await;
		let removed = store.sweep(chrono::Utc::now());
		if removed > 0 {
			tracing::debug!(removed, "cloud oauth janitor swept expired flows");
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_flow(id: Uuid, created_ago: chrono::Duration) -> OauthFlow {
		OauthFlow {
			id,
			provider_id: "mock".to_string(),
			client_id: "cid".to_string(),
			client_secret: "csec".to_string(),
			redirect_uri: "http://127.0.0.1:9".to_string(),
			state: "state".to_string(),
			pkce_verifier: "verifier".to_string(),
			created_at: chrono::Utc::now() - created_ago,
			terminal_at: None,
			status: OauthFlowStatus::Pending,
		}
	}

	#[test]
	fn test_flow_creation_sets_pending() {
		let flow = sample_flow(Uuid::new_v4(), chrono::Duration::seconds(0));
		assert!(matches!(flow.status, OauthFlowStatus::Pending));
		assert!(flow.terminal_at.is_none());
	}

	#[test]
	fn test_completed_status_wire_shape_matches_ts_type() {
		// Regression: specta does not honour #[serde(flatten)] on enum variants.
		// The TS type must see `tokens` nested, so the wire JSON must too.
		let status = OauthFlowStatus::Completed {
			tokens: crate::ops::cloud::oauth::provider::TokenSet {
				access_token: "atok".to_string(),
				refresh_token: Some("rtok".to_string()),
				expires_at: chrono::Utc::now(),
				scope: Some("Files.ReadWrite.All".to_string()),
			},
			display_name: Some("Alice".to_string()),
		};

		let json = serde_json::to_value(&status).expect("serialize");
		assert_eq!(json["type"], "completed");
		assert!(
			json.get("tokens").is_some(),
			"tokens must be a nested object, not flattened"
		);
		assert_eq!(json["tokens"]["access_token"], "atok");
		assert_eq!(json["tokens"]["refresh_token"], "rtok");
		assert_eq!(json["display_name"], "Alice");
		// Must NOT appear at top level (that would be the flattened shape).
		assert!(json.get("access_token").is_none());

		let round_trip: OauthFlowStatus =
			serde_json::from_value(json).expect("deserialize round-trip");
		assert!(matches!(round_trip, OauthFlowStatus::Completed { .. }));
	}

	#[test]
	fn test_ttl_janitor_removes_expired() {
		let store = OauthFlowStore::new();
		let (tx, _rx) = watch::channel(false);

		// 11 minutes old pending flow — must be evicted.
		let expired_id = Uuid::new_v4();
		store.insert(
			sample_flow(expired_id, chrono::Duration::minutes(11)),
			tx.clone(),
		);

		// 2 minute old pending flow — must survive.
		let alive_id = Uuid::new_v4();
		store.insert(
			sample_flow(alive_id, chrono::Duration::minutes(2)),
			tx.clone(),
		);

		// 1 minute old flow that just completed — survives (within 30s grace),
		// actually survives until grace expires: set terminal_at to now so the
		// grace period is fresh.
		let completed_id = Uuid::new_v4();
		let mut completed = sample_flow(completed_id, chrono::Duration::minutes(1));
		completed.status = OauthFlowStatus::Cancelled;
		completed.terminal_at = Some(chrono::Utc::now());
		store.insert(completed, tx);

		let removed = store.sweep(chrono::Utc::now());
		assert_eq!(
			removed, 1,
			"only the pending-expired flow should be removed"
		);
		assert!(store.status(&expired_id).is_none());
		assert!(store.status(&alive_id).is_some());
		assert!(store.status(&completed_id).is_some());
	}
}
