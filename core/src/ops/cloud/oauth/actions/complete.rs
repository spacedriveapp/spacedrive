//! Internal completion handler invoked by the loopback callback task.
//!
//! This module is deliberately not wired to the action registry — the
//! completion lifecycle runs inside the `start` task and is not a user-facing
//! operation. Keeping it off the wire reduces the attack surface and prevents
//! the UI from accidentally driving the state machine from the front end.

use super::super::{
	error::OauthError,
	flow::{ExchangeContext, OauthFlowStatus, OauthFlowStore},
	loopback::{run_loopback_callback_server, CallbackResult},
	provider::OauthProvider,
};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tokio::sync::watch;
use uuid::Uuid;

/// Default loopback wait window per RFC 8252 recommendations. Browsers that
/// take longer than five minutes to return a redirect are overwhelmingly a
/// user who walked away; the UI will show a "try again" error instead.
pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// Parameters for [`complete_flow`] — avoids an argument list longer than
/// clippy's threshold and documents the call site.
pub struct CompleteParams {
	pub flow_id: Uuid,
	pub listener: TcpListener,
	pub expected_state: String,
	pub cancel: watch::Receiver<bool>,
	pub store: OauthFlowStore,
	pub provider: Arc<dyn OauthProvider>,
}

/// Drive a flow from `Pending` to a terminal state.
///
/// Runs the loopback server, exchanges the code, fetches a display name, and
/// writes the outcome back into `store`. Never panics; all failure paths
/// transition the flow to `Failed` so the UI can surface a message.
pub async fn complete_flow(params: CompleteParams) {
	let CompleteParams {
		flow_id,
		listener,
		expected_state,
		cancel,
		store,
		provider,
	} = params;

	let callback_result = match run_loopback_callback_server(
		listener,
		expected_state,
		CALLBACK_TIMEOUT,
		cancel,
	)
	.await
	{
		Ok(result) => result,
		Err(e) => {
			tracing::warn!(%flow_id, error = %e, "oauth loopback failed");
			transition_failure(&store, &flow_id, &e);
			return;
		}
	};

	let Some(ctx) = store.exchange_context(&flow_id) else {
		// Flow vanished (cancelled mid-flight). Nothing to do.
		return;
	};

	exchange_and_store(flow_id, callback_result, ctx, store, provider).await;
}

async fn exchange_and_store(
	flow_id: Uuid,
	callback: CallbackResult,
	ctx: ExchangeContext,
	store: OauthFlowStore,
	provider: Arc<dyn OauthProvider>,
) {
	let tokens = match provider
		.exchange_code(
			&ctx.client_id,
			&ctx.client_secret,
			&ctx.redirect_uri,
			&callback.code,
			&ctx.pkce_verifier,
		)
		.await
	{
		Ok(t) => t,
		Err(e) => {
			tracing::warn!(%flow_id, error = %e, "oauth code exchange failed");
			transition_failure(&store, &flow_id, &e);
			return;
		}
	};

	// Display name is best-effort; a provider that has no user-info endpoint
	// still yields a completed flow. The UI falls back to the provider id.
	let display_name = provider
		.display_name(&tokens.access_token)
		.await
		.unwrap_or_else(|e| {
			tracing::warn!(%flow_id, error = %e, "failed to fetch display name; continuing without it");
			None
		});

	let now = chrono::Utc::now();
	store.mutate(&flow_id, |flow| {
		flow.status = OauthFlowStatus::Completed {
			tokens,
			display_name,
		};
		flow.terminal_at = Some(now);
	});
}

fn transition_failure(store: &OauthFlowStore, flow_id: &Uuid, error: &OauthError) {
	let message = error.to_string();
	let now = chrono::Utc::now();
	store.mutate(flow_id, |flow| {
		// Preserve a Cancelled status if the user raced the callback with cancel.
		if matches!(flow.status, OauthFlowStatus::Cancelled) {
			return;
		}
		flow.status = OauthFlowStatus::Failed { error: message };
		flow.terminal_at = Some(now);
	});
}
