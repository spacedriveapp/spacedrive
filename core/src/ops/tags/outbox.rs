//! Durable outbox for assertions authored against remote-owned sources.
//!
//! Tagging a file on a titan-owned source from this machine writes the
//! finished rows here, attempts delivery now, and retires them on the
//! owner's ack. Reachability changes latency, never behavior: a row waits
//! out the owner's absence, delivery is an idempotent merge, and reads
//! union pending rows so the claim shows immediately and honestly.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use super::merge::MergeAssertionsInput;
use crate::context::CoreContext;
use crate::infra::db::entities::assertion_outbox;
use crate::infra::event::Event;
use crate::library::Library;

/// Where a batch is headed: the owner and the source it owns.
pub struct RemoteTarget {
	pub device_uuid: Uuid,
	pub source_uuid: Uuid,
}

/// Author a batch durably. The rows survive a daemon restart and deliver
/// whenever the owner next answers.
pub async fn enqueue(
	library: &Library,
	target: &RemoteTarget,
	input: &MergeAssertionsInput,
) -> Result<(), sea_orm::DbErr> {
	let payload = serde_json::to_string(input)
		.map_err(|e| sea_orm::DbErr::Custom(format!("outbox payload: {e}")))?;
	assertion_outbox::ActiveModel {
		device_uuid: Set(target.device_uuid),
		source_uuid: Set(target.source_uuid),
		kind: Set("tag".to_string()),
		payload: Set(payload),
		created_at: Set(Utc::now()),
		attempts: Set(0),
		..Default::default()
	}
	.insert(library.db().conn())
	.await?;
	Ok(())
}

/// Undelivered batches for one source, oldest first, for overlaying pending
/// claims onto reads. A payload that fails to parse is reported rather than
/// silently skipped forever.
pub async fn pending_for_source(library: &Library, source_uuid: Uuid) -> Vec<MergeAssertionsInput> {
	let rows = match assertion_outbox::Entity::find()
		.filter(assertion_outbox::Column::SourceUuid.eq(source_uuid))
		.all(library.db().conn())
		.await
	{
		Ok(rows) => rows,
		Err(error) => {
			tracing::warn!(%error, "assertion outbox unavailable");
			return Vec::new();
		}
	};

	let mut inputs = Vec::with_capacity(rows.len());
	for row in rows {
		match serde_json::from_str::<MergeAssertionsInput>(&row.payload) {
			Ok(input) => inputs.push(input),
			Err(error) => tracing::warn!(row = row.id, %error, "unreadable outbox payload"),
		}
	}
	inputs
}

/// Deliver every due row once, across all open libraries. Returns how many
/// batches were acked and retired.
pub async fn drain(context: &Arc<CoreContext>) -> u64 {
	let libraries = context.libraries().await.get_open_libraries().await;
	let mut delivered = 0u64;

	for library in libraries {
		let rows = match assertion_outbox::Entity::find()
			.all(library.db().conn())
			.await
		{
			Ok(rows) => rows,
			Err(error) => {
				tracing::warn!(%error, "assertion outbox unavailable");
				continue;
			}
		};

		if !rows.is_empty() {
			tracing::debug!(rows = rows.len(), "draining assertion outbox");
		}
		let now = Utc::now();
		for row in rows {
			if row.next_attempt_at.is_some_and(|next| next > now) {
				continue;
			}
			let payload: serde_json::Value = match serde_json::from_str(&row.payload) {
				Ok(value) => value,
				Err(error) => {
					tracing::warn!(row = row.id, %error, "unreadable outbox payload");
					continue;
				}
			};

			// The server resolves its own open library; a caller cannot know
			// a peer's library ids without asking first. The timeout keeps a
			// half-open connection from parking the worker; the row simply
			// waits for the next pass.
			let delivery = tokio::time::timeout(
				Duration::from_secs(30),
				crate::service::network::protocol::remote_ops::call(
					context,
					row.device_uuid,
					"sources.assertions.merge",
					None,
					payload,
				),
			)
			.await
			.unwrap_or_else(|_| Err(anyhow::anyhow!("delivery timed out")));

			match delivery {
				Ok(_) => {
					// The reply is the ack: the owner committed before
					// answering, so the row has nothing left to say.
					if let Err(error) = assertion_outbox::Entity::delete_by_id(row.id)
						.exec(library.db().conn())
						.await
					{
						tracing::warn!(row = row.id, %error, "delivered row was not retired");
					} else {
						tracing::info!(
							source = %row.source_uuid,
							device = %row.device_uuid,
							"assertion batch delivered"
						);
						delivered += 1;
					}
				}
				Err(error) => {
					let attempts = row.attempts + 1;
					let wait = backoff(attempts);
					let mut update: assertion_outbox::ActiveModel = row.into();
					update.attempts = Set(attempts);
					update.next_attempt_at = Set(Some(now + wait));
					update.last_error = Set(Some(error.to_string()));
					if let Err(error) = update.update(library.db().conn()).await {
						tracing::warn!(%error, "outbox attempt was not recorded");
					}
				}
			}
		}
	}

	delivered
}

/// Thirty seconds doubling to half an hour: prompt after a blip, quiet
/// while a NAS spends the night off.
fn backoff(attempts: i32) -> chrono::Duration {
	let seconds = 30i64.saturating_mul(1i64 << attempts.clamp(0, 6).min(6));
	chrono::Duration::seconds(seconds.min(1800))
}

/// Start the drain worker: a connection is the moment a waiting row becomes
/// deliverable, and a slow tick covers signals that never arrive.
pub fn start(context: Arc<CoreContext>) {
	tokio::spawn(async move {
		let mut events = context.events.subscribe();
		let mut tick = tokio::time::interval(Duration::from_secs(120));
		tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
		loop {
			tokio::select! {
				event = events.recv() => {
					match event {
						Ok(Event::DeviceConnected { .. }) => {
							drain(&context).await;
						}
						Ok(_) => {}
						Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
						Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
					}
				}
				_ = tick.tick() => {
					drain(&context).await;
				}
			}
		}
	});
}
