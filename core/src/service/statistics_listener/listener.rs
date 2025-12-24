//! Per-library background listener that recalculates statistics while ResourceEvents flow

use crate::{
	infra::event::{Event, EventBus, EventSubscriber},
	library::Library,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

/// Duration to wait between recalculations while events are flowing
const RECALCULATION_INTERVAL: Duration = Duration::from_secs(5);

/// Duration to wait for new events before stopping the listener
/// If no events arrive within this window, we consider the "activity burst" finished
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Spawn a background task that recalculates library statistics periodically
/// while ResourceChanged events are flowing through the event bus
///
/// The listener uses a throttling mechanism:
/// - Recalculates at most every 5 seconds while events are flowing
/// - Stops recalculating after 10 seconds of no events
/// - Automatically restarts when new events arrive
///
/// Returns a JoinHandle that can be used to abort the listener
pub fn spawn_statistics_listener(
	library: Arc<Library>,
	event_bus: Arc<EventBus>,
) -> tokio::task::JoinHandle<()> {
	let library_id = library.id();

	tokio::spawn(async move {
		let library_name = library.name().await;

		info!(
			library_id = %library_id,
			library_name = %library_name,
			"Spawning statistics recalculation listener"
		);

		let mut subscriber = event_bus.subscribe();

		// Wait for first ResourceChanged event to start
		loop {
			match subscriber.recv().await {
				Ok(event) => {
					// Check if this library was closed
					if is_library_closed_event(&event, library_id) {
						info!(
							library_id = %library_id,
							library_name = %library_name,
							"Library closed, statistics listener shutting down"
						);
						return;
					}

					if is_resource_changed_event(&event) {
						debug!(
							library_id = %library_id,
							library_name = %library_name,
							"First ResourceChanged event detected, starting active recalculation mode"
						);
						break;
					}
				}
				Err(RecvError::Lagged(skipped)) => {
					warn!(
						library_id = %library_id,
						library_name = %library_name,
						skipped = skipped,
						"Event subscriber lagged, some events were skipped"
					);
					// Continue listening
				}
				Err(RecvError::Closed) => {
					info!(
						library_id = %library_id,
						library_name = %library_name,
						"Event bus closed, statistics listener shutting down"
					);
					return;
				}
			}
		}

		// Main loop: active recalculation while events are flowing
		loop {
			if let Err(e) =
				run_active_recalculation_cycle(&library, &mut subscriber, library_id, &library_name)
					.await
			{
				error!(
					library_id = %library_id,
					library_name = %library_name,
					error = %e,
					"Error in statistics recalculation cycle"
				);
			}

			// After an active cycle ends (idle timeout), wait for next ResourceChanged event
			debug!(
				library_id = %library_id,
				library_name = %library_name,
				"Active recalculation cycle ended, waiting for next ResourceChanged event"
			);

			loop {
				match subscriber.recv().await {
					Ok(event) => {
						// Check if this library was closed
						if is_library_closed_event(&event, library_id) {
							info!(
								library_id = %library_id,
								library_name = %library_name,
								"Library closed, statistics listener shutting down"
							);
							return;
						}

						if is_resource_changed_event(&event) {
							debug!(
								library_id = %library_id,
								library_name = %library_name,
								"New ResourceChanged event detected, restarting active recalculation"
							);
							break; // Restart active recalculation
						}
					}
					Err(RecvError::Lagged(skipped)) => {
						warn!(
							library_id = %library_id,
							library_name = %library_name,
							skipped = skipped,
							"Event subscriber lagged during idle wait"
						);
					}
					Err(RecvError::Closed) => {
						info!(
							library_id = %library_id,
							library_name = %library_name,
							"Event bus closed, statistics listener shutting down"
						);
						return;
					}
				}
			}
		}
	})
}

/// Run one active recalculation cycle while events are flowing
///
/// Recalculates statistics every 5 seconds while ResourceChanged events arrive.
/// Returns when no events have been received for 10 seconds (idle timeout).
async fn run_active_recalculation_cycle(
	library: &Arc<Library>,
	subscriber: &mut EventSubscriber,
	library_id: uuid::Uuid,
	library_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let mut recalc_interval = interval(RECALCULATION_INTERVAL);
	recalc_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

	// Trigger immediate recalculation at start of active cycle
	if let Err(e) = library.recalculate_statistics().await {
		warn!(
			library_id = %library_id,
			library_name = %library_name,
			error = %e,
			"Failed to trigger statistics recalculation"
		);
	} else {
		debug!(
			library_id = %library_id,
			library_name = %library_name,
			"Triggered statistics recalculation at start of active cycle"
		);
	}

	let mut last_event_time = tokio::time::Instant::now();
	let mut event_count = 0u64;

	loop {
		tokio::select! {
			// Check for idle timeout
			_ = sleep(IDLE_TIMEOUT) => {
				let elapsed_since_last_event = tokio::time::Instant::now() - last_event_time;
				if elapsed_since_last_event >= IDLE_TIMEOUT {
					info!(
						library_id = %library_id,
						library_name = %library_name,
						event_count = event_count,
						"No events for {} seconds, ending active recalculation cycle",
						IDLE_TIMEOUT.as_secs()
					);
					break; // End active cycle
				}
			}

			// Recalculation interval tick
			_ = recalc_interval.tick() => {
				if let Err(e) = library.recalculate_statistics().await {
					warn!(
						library_id = %library_id,
						library_name = %library_name,
						error = %e,
						"Failed to trigger periodic statistics recalculation"
					);
				} else {
					debug!(
						library_id = %library_id,
						library_name = %library_name,
						event_count = event_count,
						"Triggered periodic statistics recalculation"
					);
				}
			}

			// Listen for events
			result = subscriber.recv() => {
				match result {
					Ok(event) => {
						// Check if this library was closed
						if is_library_closed_event(&event, library_id) {
							info!(
								library_id = %library_id,
								library_name = %library_name,
								"Library closed during active recalculation"
							);
							return Err("Library closed".into());
						}

						if is_resource_changed_event(&event) {
							last_event_time = tokio::time::Instant::now();
							event_count += 1;

							// Log every 100 events to show activity without spam
							if event_count % 100 == 0 {
								debug!(
									library_id = %library_id,
									library_name = %library_name,
									event_count = event_count,
									"Processed {} ResourceChanged events in this cycle",
									event_count
								);
							}
						}
					}
					Err(RecvError::Lagged(skipped)) => {
						warn!(
							library_id = %library_id,
							library_name = %library_name,
							skipped = skipped,
							"Event subscriber lagged during active recalculation"
						);
						last_event_time = tokio::time::Instant::now();
					}
					Err(RecvError::Closed) => {
						info!(
							library_id = %library_id,
							library_name = %library_name,
							"Event bus closed during active recalculation"
						);
						return Err("Event bus closed".into());
					}
				}
			}
		}
	}

	Ok(())
}

/// Check if an event is a ResourceChanged event
fn is_resource_changed_event(event: &Event) -> bool {
	matches!(
		event,
		Event::ResourceChanged { .. } | Event::ResourceChangedBatch { .. }
	)
}

/// Check if an event is a LibraryClosed event for the specified library
fn is_library_closed_event(event: &Event, library_id: uuid::Uuid) -> bool {
	matches!(event, Event::LibraryClosed { id, .. } if *id == library_id)
}
