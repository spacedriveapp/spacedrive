mod common;

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use uuid::Uuid;

use sd_client::{
	BrokerOptions, BrokerSubscription, DaemonRequest, DaemonResponse, Event, EventFilter,
	SubscriptionBroker,
};

use common::{wait_until, MockDaemon};

fn test_options() -> BrokerOptions {
	BrokerOptions {
		linger: Duration::from_millis(300),
		initial_backoff: Duration::from_millis(50),
		max_backoff: Duration::from_millis(400),
		channel_capacity: 8,
	}
}

fn library_opened(id: Uuid, name: &str) -> Event {
	Event::LibraryOpened {
		id,
		name: name.to_string(),
		path: PathBuf::from("/tmp/lib"),
	}
}

fn empty_filter() -> EventFilter {
	EventFilter {
		library_id: None,
		job_id: None,
		device_id: None,
		resource_type: None,
		path_scope: None,
		include_descendants: None,
	}
}

async fn recv(sub: &mut BrokerSubscription) -> Event {
	tokio::time::timeout(Duration::from_secs(2), sub.recv())
		.await
		.expect("timed out waiting for event")
		.expect("subscription closed")
}

#[tokio::test]
async fn fan_out_shares_one_connection() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let mut subs: Vec<_> = (0..3)
		.map(|_| broker.subscribe(vec!["LibraryOpened".into()], None))
		.collect();

	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;
	assert_eq!(
		mock.subscribe_count(),
		1,
		"one Subscribe for the shared key"
	);
	assert_eq!(broker.connection_count(), 1);

	let id = Uuid::new_v4();
	mock.emit(library_opened(id, "shared"));

	for sub in &mut subs {
		match recv(sub).await {
			Event::LibraryOpened { id: got, .. } => assert_eq!(got, id),
			other => panic!("unexpected event: {other:?}"),
		}
	}
}

#[tokio::test]
async fn equivalent_requests_reuse_one_connection() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let _a = broker.subscribe(vec!["Refresh".into(), "LibraryOpened".into()], None);
	let _b = broker.subscribe(
		vec!["LibraryOpened".into(), "Refresh".into(), "Refresh".into()],
		Some(empty_filter()),
	);

	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;
	assert_eq!(mock.subscribe_count(), 1);
	assert_eq!(broker.connection_count(), 1);
}

#[tokio::test]
async fn distinct_filters_use_distinct_connections() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let mut by_type = broker.subscribe(vec!["LibraryOpened".into()], None);
	let mut refresh_only = broker.subscribe(vec!["Refresh".into()], None);
	let mut by_library = empty_filter();
	let library_id = Uuid::new_v4();
	by_library.library_id = Some(library_id);
	let mut filtered = broker.subscribe(vec!["LibraryOpened".into()], Some(by_library));

	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 3
	})
	.await;
	assert_eq!(mock.subscribe_count(), 3);
	assert_eq!(broker.connection_count(), 3);

	let records = mock.subscribes();
	let connections: std::collections::HashSet<_> = records.iter().map(|r| r.connection).collect();
	assert_eq!(connections.len(), 3, "each key gets its own connection");

	mock.emit(Event::Refresh);
	assert!(matches!(recv(&mut refresh_only).await, Event::Refresh));

	mock.emit(library_opened(library_id, "mine"));
	assert!(matches!(
		recv(&mut by_type).await,
		Event::LibraryOpened { .. }
	));
	assert!(matches!(
		recv(&mut filtered).await,
		Event::LibraryOpened { .. }
	));

	// The Refresh went only to the connection subscribed to it.
	assert!(by_type.try_recv().is_none());
}

#[tokio::test]
async fn last_drop_lingers_then_closes_the_connection() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let sub = broker.subscribe(vec!["Refresh".into()], None);
	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;
	drop(sub);

	// Well inside the linger period the connection is still up.
	tokio::time::sleep(Duration::from_millis(100)).await;
	assert_eq!(mock.open_subscription_count(), 1);
	assert_eq!(broker.connection_count(), 1);

	// A remount inside the linger period reuses it: no second Subscribe.
	let sub = broker.subscribe(vec!["Refresh".into()], None);
	tokio::time::sleep(Duration::from_millis(400)).await;
	assert_eq!(mock.subscribe_count(), 1);
	assert_eq!(mock.open_subscription_count(), 1);
	drop(sub);

	// With no reclaim, the linger elapses and the connection closes.
	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 0
	})
	.await;
	assert_eq!(broker.connection_count(), 0);
}

#[tokio::test]
async fn reconnect_resubscribes_and_tolerates_replay_duplicates() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let mut sub = broker.subscribe(vec!["LibraryOpened".into()], None);
	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;

	let id = Uuid::new_v4();
	mock.emit(library_opened(id, "before"));
	assert!(matches!(recv(&mut sub).await, Event::LibraryOpened { .. }));

	// The daemon dies mid-stream; its replay buffer will resend the last
	// event on resubscribe.
	mock.set_replay(vec![library_opened(id, "before")]);
	mock.kill_subscriptions();

	wait_until(Duration::from_secs(2), || mock.subscribe_count() == 2).await;

	// The replayed event arrives again: duplicates across reconnect are
	// expected and must be tolerated by subscribers.
	match recv(&mut sub).await {
		Event::LibraryOpened { name, .. } => assert_eq!(name, "before"),
		other => panic!("unexpected event: {other:?}"),
	}

	mock.emit(library_opened(id, "after"));
	match recv(&mut sub).await {
		Event::LibraryOpened { name, .. } => assert_eq!(name, "after"),
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test]
async fn reconnect_backs_off_exponentially_and_recovers() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let mut sub = broker.subscribe(vec!["LibraryOpened".into()], None);
	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;

	let accepted_before = mock.accept_times().len();
	mock.set_refuse(true);
	mock.kill_subscriptions();

	// Watch three refused reconnect attempts land.
	wait_until(Duration::from_secs(5), || {
		mock.accept_times().len() >= accepted_before + 3
	})
	.await;

	let attempts = &mock.accept_times()[accepted_before..];
	let first_gap = attempts[1] - attempts[0];
	let second_gap = attempts[2] - attempts[1];
	// Backoff doubles after each failed attempt (50ms -> 100ms -> 200ms).
	// Sleeps guarantee at-least timing, so only lower bounds are asserted.
	assert!(
		first_gap >= Duration::from_millis(90),
		"first gap {first_gap:?}"
	);
	assert!(
		second_gap >= Duration::from_millis(180),
		"second gap {second_gap:?}"
	);
	assert!(second_gap > first_gap);

	// Once the daemon answers again, the broker resubscribes and resumes.
	mock.set_refuse(false);
	wait_until(Duration::from_secs(5), || mock.subscribe_count() == 2).await;

	mock.emit(library_opened(Uuid::new_v4(), "recovered"));
	match recv(&mut sub).await {
		Event::LibraryOpened { name, .. } => assert_eq!(name, "recovered"),
		other => panic!("unexpected event: {other:?}"),
	}
}

#[tokio::test]
async fn slow_subscriber_drops_oldest_without_blocking_peers() {
	let mock = MockDaemon::start().await;
	let broker = SubscriptionBroker::with_options(mock.addr(), test_options());

	let mut fast = broker.subscribe(vec!["LibraryOpened".into()], None);
	let mut slow = broker.subscribe(vec!["LibraryOpened".into()], None);
	wait_until(Duration::from_secs(2), || {
		mock.open_subscription_count() == 1
	})
	.await;

	// The fast subscriber keeps up with all 20 events while the slow one
	// never reads; delivery to fast proves the slow one blocked nothing.
	for i in 0..20 {
		mock.emit(library_opened(Uuid::new_v4(), &i.to_string()));
		match recv(&mut fast).await {
			Event::LibraryOpened { name, .. } => assert_eq!(name, i.to_string()),
			other => panic!("unexpected event: {other:?}"),
		}
	}

	// The slow subscriber lost the oldest events and retains exactly the
	// ring capacity (8) newest ones.
	let mut backlog = Vec::new();
	while let Some(event) = slow.try_recv() {
		match event {
			Event::LibraryOpened { name, .. } => backlog.push(name),
			other => panic!("unexpected event: {other:?}"),
		}
	}
	let expected: Vec<String> = (12..20).map(|i| i.to_string()).collect();
	assert_eq!(backlog, expected);
}

/// Direct protocol check of the daemon behavior the broker is built around:
/// a second `Subscribe` on one connection replaces the first, so a
/// connection can never serve two filters at once.
#[tokio::test]
async fn second_subscribe_on_a_connection_replaces_the_first() {
	let mock = MockDaemon::start().await;

	let mut stream = TcpStream::connect(mock.addr()).await.unwrap();
	let encode = |request: DaemonRequest| serde_json::to_string(&request).unwrap() + "\n";

	let first = encode(DaemonRequest::Subscribe {
		event_types: vec!["LibraryOpened".into()],
		filter: None,
	});
	stream.write_all(first.as_bytes()).await.unwrap();

	let (reader, mut writer) = stream.into_split();
	let mut lines = BufReader::new(reader).lines();

	let ack = lines.next_line().await.unwrap().unwrap();
	assert!(matches!(
		serde_json::from_str::<DaemonResponse>(&ack).unwrap(),
		DaemonResponse::Subscribed
	));

	let second = encode(DaemonRequest::Subscribe {
		event_types: vec!["Refresh".into()],
		filter: None,
	});
	writer.write_all(second.as_bytes()).await.unwrap();
	let ack = lines.next_line().await.unwrap().unwrap();
	assert!(matches!(
		serde_json::from_str::<DaemonResponse>(&ack).unwrap(),
		DaemonResponse::Subscribed
	));

	// Same connection, two Subscribe records, one live subscription.
	let records = mock.subscribes();
	assert_eq!(records.len(), 2);
	assert_eq!(records[0].connection, records[1].connection);
	assert_eq!(mock.open_subscription_count(), 1);

	// An event matching only the first subscription no longer arrives; the
	// next frame on the wire is the event matching the replacement.
	mock.emit(library_opened(Uuid::new_v4(), "replaced"));
	mock.emit(Event::Refresh);

	let frame = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
		.await
		.expect("timed out waiting for frame")
		.unwrap()
		.unwrap();
	assert!(matches!(
		serde_json::from_str::<DaemonResponse>(&frame).unwrap(),
		DaemonResponse::Event(Event::Refresh)
	));
}
