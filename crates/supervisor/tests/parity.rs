#![cfg(unix)]

use sd_supervisor::{
	probe_http, resolve_port_assignments, ContainerRuntime, PortLedger, PortLedgerStore,
	PortRange, PortRequest, PortResolution, ServiceDefinition, ServiceKind, ServiceLogs,
	ProcessState, Supervisor, SupervisorConfig, SupervisorError, Timing,
};
use sd_supervisor::ledger::{LeaseSource, PortLease, PortLeaseHolder, PortPolicy};
use sd_supervisor::spec::{HealthState, Ownership};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpListener,
	task::JoinHandle,
	time::{sleep, timeout, Instant},
};

fn test_timing() -> Timing {
	Timing {
		health_interval: Duration::from_millis(25),
		adopted_death_threshold: 3,
		fast_exit: Duration::from_millis(500),
		max_fast_exits: 5,
		backoff_base: Duration::from_millis(10),
		backoff_cap: Duration::from_millis(50),
		stop_grace: Duration::from_millis(200),
	}
}

fn supervisor_in(dir: &std::path::Path, timing: Timing) -> Supervisor {
	Supervisor::new(SupervisorConfig {
		logs_dir: dir.join("logs"),
		ledger_path: dir.join("ports.json"),
		timing,
		containers: ContainerRuntime::default(),
	})
}

fn daemon(name: &str, script: &str, health: &str) -> ServiceDefinition {
	daemon_env(name, script, health, BTreeMap::new())
}

fn daemon_env(
	name: &str,
	script: &str,
	health: &str,
	env: BTreeMap<String, String>,
) -> ServiceDefinition {
	ServiceDefinition {
		name: name.to_string(),
		kind: ServiceKind::Daemon {
			command: vec!["/bin/sh".into(), "-c".into(), script.into()],
			cwd: PathBuf::from("/tmp"),
			env,
			health: health.to_string(),
			dev: false,
		},
	}
}

/// A health endpoint no server answers: connection refused, immediately.
const DEAD_HEALTH: &str = "http://127.0.0.1:1/health";

/// Serve a fixed HTTP status until the returned handle is aborted.
async fn health_server(status: u16) -> (String, JoinHandle<()>) {
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	let handle = tokio::spawn(async move {
		loop {
			let Ok((mut socket, _)) = listener.accept().await else {
				break;
			};
			tokio::spawn(async move {
				let mut buffer = [0u8; 1024];
				let _ = socket.read(&mut buffer).await;
				let response = format!(
					"HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
				);
				let _ = socket.write_all(response.as_bytes()).await;
			});
		}
	});
	(format!("http://{addr}/health"), handle)
}

async fn wait_for<F: Fn(&sd_supervisor::ProcessStatus) -> bool>(
	supervisor: &Supervisor,
	name: &str,
	deadline: Duration,
	predicate: F,
) -> sd_supervisor::ProcessStatus {
	let started = Instant::now();
	loop {
		let status = supervisor.status_of(name).await.unwrap();
		if predicate(&status) {
			return status;
		}
		if started.elapsed() > deadline {
			panic!("condition not reached within {deadline:?}; last status: {status:?}");
		}
		sleep(Duration::from_millis(10)).await;
	}
}

#[test]
fn production_timing_matches_the_spec() {
	let timing = Timing::default();
	assert_eq!(timing.health_interval, Duration::from_secs(30));
	assert_eq!(timing.adopted_death_threshold, 3);
	assert_eq!(timing.fast_exit, Duration::from_secs(10));
	assert_eq!(timing.max_fast_exits, 5);
	assert_eq!(timing.backoff_base, Duration::from_secs(1));
	assert_eq!(timing.backoff_cap, Duration::from_secs(60));
	assert_eq!(timing.stop_grace, Duration::from_secs(10));
}

#[test]
fn backoff_doubles_per_fast_exit_and_caps() {
	let timing = Timing::default();
	assert_eq!(timing.backoff(0), Duration::from_secs(1));
	assert_eq!(timing.backoff(1), Duration::from_secs(2));
	assert_eq!(timing.backoff(2), Duration::from_secs(4));
	assert_eq!(timing.backoff(5), Duration::from_secs(32));
	assert_eq!(timing.backoff(6), Duration::from_secs(60));
	assert_eq!(timing.backoff(30), Duration::from_secs(60));
}

#[tokio::test]
async fn already_healthy_daemon_is_adopted_not_spawned() {
	let dir = tempfile::tempdir().unwrap();
	let marker = dir.path().join("spawned");
	let (health, server) = health_server(200).await;
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon(
			"svc",
			&format!("touch {} && sleep 30", marker.display()),
			&health,
		))
		.await
		.unwrap();

	let status = supervisor.start("svc").await.unwrap();
	assert_eq!(status.state, ProcessState::Running);
	assert_eq!(status.ownership, Some(Ownership::Adopted));
	assert_eq!(status.health, HealthState::Ok);
	assert_eq!(status.pid, None);
	assert!(!marker.exists(), "adoption must not spawn a child");
	server.abort();
}

#[tokio::test]
async fn start_on_a_running_service_changes_nothing() {
	let dir = tempfile::tempdir().unwrap();
	let (health, server) = health_server(200).await;
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon("svc", "sleep 30", &health))
		.await
		.unwrap();

	let first = supervisor.start("svc").await.unwrap();
	let second = supervisor.start("svc").await.unwrap();
	assert_eq!(first, second);
	server.abort();
}

#[tokio::test]
async fn crash_loop_gives_up_after_the_fast_exit_cap() {
	let dir = tempfile::tempdir().unwrap();
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon("svc", "exit 1", DEAD_HEALTH))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	let status = wait_for(&supervisor, "svc", Duration::from_secs(5), |s| {
		s.state == ProcessState::Failed
	})
	.await;
	assert_eq!(status.health, HealthState::Unknown);
	assert!(
		status.detail.contains("gave up after 5 fast exits"),
		"detail: {}",
		status.detail
	);
	assert_eq!(status.restarts, 4);
}

#[tokio::test]
async fn slow_exits_reset_the_fast_exit_counter() {
	let dir = tempfile::tempdir().unwrap();
	let mut timing = test_timing();
	timing.fast_exit = Duration::from_millis(100);
	let supervisor = supervisor_in(dir.path(), timing);
	supervisor
		.register(daemon("svc", "sleep 0.3; exit 1", DEAD_HEALTH))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	let status = wait_for(&supervisor, "svc", Duration::from_secs(5), |s| {
		s.restarts >= 3
	})
	.await;
	assert_ne!(
		status.state,
		ProcessState::Failed,
		"slow exits must never trip the fast-exit cap"
	);
	// Every restart delay stays at the base backoff, because the counter
	// reset on each slow exit.
	assert!(
		status.detail.contains("restarting in 10ms") || status.detail.contains("spawned pid"),
		"detail: {}",
		status.detail
	);
	supervisor.stop("svc").await.unwrap();
}

#[tokio::test]
async fn unhealthy_adopted_service_is_respawned_as_owned() {
	let dir = tempfile::tempdir().unwrap();
	let (health, server) = health_server(200).await;
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon("svc", "sleep 30", &health))
		.await
		.unwrap();

	supervisor.up().await;
	let adopted = supervisor.status_of("svc").await.unwrap();
	assert_eq!(adopted.ownership, Some(Ownership::Adopted));

	server.abort();
	let status = wait_for(&supervisor, "svc", Duration::from_secs(5), |s| {
		s.ownership == Some(Ownership::Owned)
	})
	.await;
	assert!(status.pid.is_some());
	supervisor.shutdown();
	supervisor.stop("svc").await.unwrap();
}

#[tokio::test]
async fn stopping_an_adopted_service_is_refused() {
	let dir = tempfile::tempdir().unwrap();
	let (health, server) = health_server(200).await;
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon("svc", "sleep 30", &health))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	let err = supervisor.stop("svc").await.unwrap_err();
	assert!(matches!(err, SupervisorError::AdoptedService(_)));
	server.abort();
}

#[tokio::test]
async fn stop_escalates_to_kill_after_the_grace_period() {
	let dir = tempfile::tempdir().unwrap();
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon("svc", "trap '' TERM; sleep 30", DEAD_HEALTH))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	wait_for(&supervisor, "svc", Duration::from_secs(2), |s| {
		s.pid.is_some()
	})
	.await;
	// The trap needs to be installed before the stop signal arrives.
	sleep(Duration::from_millis(150)).await;

	let started = Instant::now();
	let status = timeout(Duration::from_secs(3), supervisor.stop("svc"))
		.await
		.expect("stop must finish once the grace period escalates")
		.unwrap();
	assert_eq!(status.state, ProcessState::Stopped);
	assert!(
		started.elapsed() >= Duration::from_millis(200),
		"a child ignoring the stop signal is only killed after the grace period"
	);
}

#[tokio::test]
async fn stop_during_restart_backoff_cancels_the_respawn() {
	let dir = tempfile::tempdir().unwrap();
	let mut timing = test_timing();
	timing.backoff_base = Duration::from_secs(5);
	timing.backoff_cap = Duration::from_secs(5);
	let supervisor = supervisor_in(dir.path(), timing);
	supervisor
		.register(daemon("svc", "exit 1", DEAD_HEALTH))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	wait_for(&supervisor, "svc", Duration::from_secs(2), |s| {
		s.state == ProcessState::Starting && s.pid.is_none()
	})
	.await;

	let status = supervisor.stop("svc").await.unwrap();
	assert_eq!(status.state, ProcessState::Stopped);
	sleep(Duration::from_millis(200)).await;
	let after = supervisor.status_of("svc").await.unwrap();
	assert_eq!(after.state, ProcessState::Stopped, "the respawn was cancelled");
	assert_eq!(after.pid, None);
}

#[tokio::test]
async fn dev_server_exits_are_stops_never_crashes() {
	let dir = tempfile::tempdir().unwrap();
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(ServiceDefinition {
			name: "dev".to_string(),
			kind: ServiceKind::Daemon {
				command: vec!["/bin/sh".into(), "-c".into(), "exit 0".into()],
				cwd: PathBuf::from("/tmp"),
				env: BTreeMap::new(),
				health: DEAD_HEALTH.to_string(),
				dev: true,
			},
		})
		.await
		.unwrap();

	supervisor.start("dev").await.unwrap();
	let status = wait_for(&supervisor, "dev", Duration::from_secs(2), |s| {
		s.state == ProcessState::Stopped
	})
	.await;
	assert!(
		status.detail.contains("dev servers are not respawned"),
		"detail: {}",
		status.detail
	);
	assert_eq!(status.restarts, 0);
}

#[tokio::test]
async fn spawned_children_inherit_and_override_environment() {
	let dir = tempfile::tempdir().unwrap();
	std::env::set_var("SD_SUPERVISOR_TEST_INHERIT", "inherited");
	let supervisor = supervisor_in(dir.path(), test_timing());
	supervisor
		.register(daemon_env(
			"svc",
			"echo \"I=${SD_SUPERVISOR_TEST_INHERIT:-unset} S=${SD_SUPERVISOR_TEST_SET:-unset}\"; sleep 30",
			DEAD_HEALTH,
			BTreeMap::from([("SD_SUPERVISOR_TEST_SET".to_string(), "set".to_string())]),
		))
		.await
		.unwrap();

	supervisor.start("svc").await.unwrap();
	sleep(Duration::from_millis(300)).await;
	let lines = supervisor.logs("svc", 10).await.unwrap();
	assert!(
		lines.iter().any(|line| line == "I=inherited S=set"),
		"lines: {lines:?}"
	);
	supervisor.stop("svc").await.unwrap();
}

#[tokio::test]
async fn probe_reports_status_not_errors() {
	let (health, server) = health_server(500).await;
	let failing = probe_http(&health, None).await;
	assert!(!failing.ok);
	assert_eq!(failing.detail, "HTTP 500");
	server.abort();

	let (health, server) = health_server(204).await;
	let ok = probe_http(&health, None).await;
	assert!(ok.ok);
	assert_eq!(ok.detail, "HTTP 204");
	server.abort();

	let refused = probe_http(DEAD_HEALTH, None).await;
	assert!(!refused.ok);
}

#[test]
fn log_tail_reads_only_the_end_of_the_file() {
	let dir = tempfile::tempdir().unwrap();
	let logs = ServiceLogs::new(dir.path());
	logs.init().unwrap();
	std::fs::write(
		logs.path("svc"),
		"one\ntwo\nthree\nfour\n",
	)
	.unwrap();
	assert_eq!(logs.tail("svc", 2).unwrap(), vec!["three", "four"]);
	assert_eq!(logs.tail("missing", 2).unwrap(), Vec::<String>::new());
}

#[test]
fn log_rotation_copies_aside_and_truncates_in_place() {
	let dir = tempfile::tempdir().unwrap();
	let logs = ServiceLogs::new(dir.path());
	logs.init().unwrap();
	let line = "x".repeat(1023) + "\n";
	let body = line.repeat(17 * 1024);
	std::fs::write(logs.path("svc"), &body).unwrap();

	assert!(logs.rotate("svc").unwrap());
	let rotated = dir.path().join("svc.log.1");
	assert_eq!(std::fs::metadata(&rotated).unwrap().len(), body.len() as u64);
	assert_eq!(std::fs::metadata(logs.path("svc")).unwrap().len(), 0);
	assert!(!logs.rotate("svc").unwrap());
}

fn ledger_with(leases: Vec<PortLease>, excluded: Vec<PortRange>) -> PortLedger {
	PortLedger {
		allocatable: vec![PortRange {
			from: 3000,
			to: 3005,
		}],
		excluded,
		leases,
		..PortLedger::default()
	}
}

fn request(id: &str, policy: PortPolicy, preferred: Option<u16>) -> PortRequest {
	PortRequest {
		installation_id: id.to_string(),
		endpoint_id: "web".to_string(),
		policy,
		preferred_port: preferred,
	}
}

#[test]
fn allocation_avoids_excluded_ranges() {
	let ledger = ledger_with(
		vec![],
		vec![PortRange {
			from: 3000,
			to: 3001,
		}],
	);
	let resolution = resolve_port_assignments(
		&ledger,
		&[request("app", PortPolicy::Remappable, Some(3000))],
	);
	match resolution {
		PortResolution::Ok { assignments, .. } => assert_eq!(assignments[0].port, 3002),
		PortResolution::Conflict(conflicts) => panic!("unexpected conflicts: {conflicts:?}"),
	}
}

#[test]
fn an_existing_lease_is_preserved_verbatim() {
	let ledger = ledger_with(
		vec![PortLease {
			port: 3004,
			holder: PortLeaseHolder {
				installation_id: "app".to_string(),
				endpoint_id: "web".to_string(),
			},
			policy: PortPolicy::Remappable,
			source: LeaseSource::Allocated,
		}],
		vec![],
	);
	let resolution = resolve_port_assignments(
		&ledger,
		&[request("app", PortPolicy::Remappable, Some(3000))],
	);
	match resolution {
		PortResolution::Ok {
			assignments,
			ledger: next,
		} => {
			assert_eq!(assignments[0].port, 3004);
			assert_eq!(next.generation, ledger.generation + 1);
			assert_eq!(next.leases.len(), 1);
		}
		PortResolution::Conflict(conflicts) => panic!("unexpected conflicts: {conflicts:?}"),
	}
}

#[test]
fn a_blocked_fixed_request_refuses_the_whole_batch() {
	let ledger = ledger_with(
		vec![PortLease {
			port: 3000,
			holder: PortLeaseHolder {
				installation_id: "other".to_string(),
				endpoint_id: "web".to_string(),
			},
			policy: PortPolicy::Fixed,
			source: LeaseSource::Allocated,
		}],
		vec![],
	);
	let resolution = resolve_port_assignments(
		&ledger,
		&[
			request("app", PortPolicy::Fixed, Some(3000)),
			request("friend", PortPolicy::Remappable, None),
		],
	);
	let PortResolution::Conflict(conflicts) = resolution else {
		panic!("a taken fixed port must refuse");
	};
	assert!(conflicts[0].contains("leased to other/web"), "{conflicts:?}");
}

#[test]
fn fixed_requests_outrank_remappable_preferences() {
	let ledger = ledger_with(vec![], vec![]);
	let resolution = resolve_port_assignments(
		&ledger,
		&[
			request("zeta", PortPolicy::Remappable, Some(3000)),
			request("alpha", PortPolicy::Fixed, Some(3000)),
		],
	);
	match resolution {
		PortResolution::Ok { assignments, .. } => {
			let port_of = |id: &str| {
				assignments
					.iter()
					.find(|a| a.installation_id == id)
					.unwrap()
					.port
			};
			assert_eq!(port_of("alpha"), 3000);
			assert_eq!(port_of("zeta"), 3001);
		}
		PortResolution::Conflict(conflicts) => panic!("unexpected conflicts: {conflicts:?}"),
	}
}

#[tokio::test]
async fn the_ledger_store_persists_and_reloads() {
	let dir = tempfile::tempdir().unwrap();
	let path = dir.path().join("ports.json");
	let store = PortLedgerStore::load(&path);
	let assignments = store
		.allocate(&[request("app", PortPolicy::Remappable, None)])
		.await
		.unwrap();
	assert_eq!(assignments[0].port, 3000);

	let reloaded = PortLedgerStore::load(&path);
	let ledger = reloaded.ledger().await;
	assert_eq!(ledger.generation, 1);
	assert_eq!(ledger.leases.len(), 1);
	assert_eq!(ledger.leases[0].port, 3000);
	assert_eq!(ledger.schema_version, "port-ledger/1");
}
