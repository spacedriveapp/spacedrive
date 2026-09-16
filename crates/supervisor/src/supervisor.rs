use crate::{
	compose::{compose_logs, compose_restart, compose_stop, compose_up},
	container::{ensure_container_runtime, ContainerRuntime},
	ledger::PortLedgerStore,
	logs::ServiceLogs,
	probe::{probe_http, ProbeResult},
	spec::{
		HealthState, Ownership, ProcessState, ProcessStatus, ServiceDefinition, ServiceKind,
		SpawnSpec,
	},
	timing::Timing,
};
use chrono::{DateTime, SecondsFormat, Utc};
use std::{collections::BTreeMap, path::PathBuf, process::Stdio, sync::Arc, time::Instant};
use tokio::{
	process::Command,
	sync::{broadcast, mpsc, watch, Mutex},
	task::JoinHandle,
	time::sleep,
};
use tracing::{error, info, warn};

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
	#[error("no service named {0} is registered")]
	UnknownService(String),
	#[error("a service named {0} is already registered")]
	DuplicateService(String),
	#[error("{0} is observed, not owned — its lifecycle belongs to the host that runs it")]
	ObservedService(String),
	#[error("{0} was started outside the supervisor; stop it with its own controller and the supervisor will notice")]
	AdoptedService(String),
	#[error("{0}")]
	Runtime(String),
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
	/// Directory service logs are written into.
	pub logs_dir: PathBuf,
	/// Path of the host's port ledger file.
	pub ledger_path: PathBuf,
	pub timing: Timing,
	pub containers: ContainerRuntime,
}

enum KillSignal {
	Term,
	Kill,
}

struct ChildHandle {
	exited: watch::Receiver<bool>,
	kill: mpsc::UnboundedSender<KillSignal>,
}

struct ServiceCell {
	kind: ServiceKind,
	state: ProcessState,
	ownership: Option<Ownership>,
	health: HealthState,
	pid: Option<u32>,
	since: Option<DateTime<Utc>>,
	restarts: u32,
	detail: String,
	fast_exits: u32,
	probe_failures: u32,
	stop_requested: bool,
	spawned_at: Option<Instant>,
	/// Bumped on every spawn; exit handling for a superseded child is a no-op.
	generation: u64,
	child: Option<ChildHandle>,
	restart_task: Option<JoinHandle<()>>,
}

impl ServiceCell {
	fn new(kind: ServiceKind) -> Self {
		Self {
			kind,
			state: ProcessState::Stopped,
			ownership: None,
			health: HealthState::Unknown,
			pid: None,
			since: None,
			restarts: 0,
			detail: String::new(),
			fast_exits: 0,
			probe_failures: 0,
			stop_requested: false,
			spawned_at: None,
			generation: 0,
			child: None,
			restart_task: None,
		}
	}

	fn status(&self, name: &str) -> ProcessStatus {
		ProcessStatus {
			name: name.to_string(),
			state: self.state,
			ownership: self.ownership,
			health: self.health,
			pid: self.pid,
			since: self
				.since
				.map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true)),
			restarts: self.restarts,
			detail: self.detail.clone(),
			dev: self.kind.is_dev(),
		}
	}

	fn mark_stopped(&mut self) {
		self.state = ProcessState::Stopped;
		self.health = HealthState::Unknown;
		self.ownership = None;
		self.pid = None;
		self.since = None;
		self.detail = String::new();
	}

	fn adopt(&mut self, pid: Option<u32>, detail: String) {
		self.ownership = Some(Ownership::Adopted);
		self.state = ProcessState::Running;
		self.health = HealthState::Ok;
		self.pid = pid;
		self.since = Some(Utc::now());
		self.detail = detail;
		self.probe_failures = 0;
	}
}

struct Inner {
	timing: Timing,
	logs: ServiceLogs,
	ledger: PortLedgerStore,
	containers: ContainerRuntime,
	services: Mutex<BTreeMap<String, ServiceCell>>,
	/// Every compose service addresses one runtime, so the first to start
	/// brings it up and the rest wait on that same attempt instead of racing
	/// to boot a VM. A failed attempt is not cached: the next start tries
	/// again rather than inheriting a refusal from whenever the machine last
	/// looked unready.
	containers_ready: Mutex<bool>,
	status_tx: broadcast::Sender<ProcessStatus>,
	health_task: std::sync::Mutex<Option<JoinHandle<()>>>,
}

/// The host's process supervisor: a registry of services started adopt-first,
/// probed on a fixed cadence, and restarted with backoff when owned children
/// crash.
#[derive(Clone)]
pub struct Supervisor {
	inner: Arc<Inner>,
}

impl Supervisor {
	pub fn new(config: SupervisorConfig) -> Self {
		let (status_tx, _) = broadcast::channel(256);
		Self {
			inner: Arc::new(Inner {
				timing: config.timing,
				logs: ServiceLogs::new(config.logs_dir),
				ledger: PortLedgerStore::load(config.ledger_path),
				containers: config.containers,
				services: Mutex::new(BTreeMap::new()),
				containers_ready: Mutex::new(false),
				status_tx,
				health_task: std::sync::Mutex::new(None),
			}),
		}
	}

	pub fn timing(&self) -> &Timing {
		&self.inner.timing
	}

	pub fn ledger(&self) -> &PortLedgerStore {
		&self.inner.ledger
	}

	/// State changes for every service, as they happen.
	pub fn subscribe(&self) -> broadcast::Receiver<ProcessStatus> {
		self.inner.status_tx.subscribe()
	}

	pub async fn register(&self, definition: ServiceDefinition) -> Result<(), SupervisorError> {
		let mut services = self.inner.services.lock().await;
		if services.contains_key(&definition.name) {
			return Err(SupervisorError::DuplicateService(definition.name));
		}
		services.insert(definition.name, ServiceCell::new(definition.kind));
		Ok(())
	}

	pub async fn definitions(&self) -> Vec<ServiceDefinition> {
		let services = self.inner.services.lock().await;
		services
			.iter()
			.map(|(name, cell)| ServiceDefinition {
				name: name.clone(),
				kind: cell.kind.clone(),
			})
			.collect()
	}

	pub async fn status(&self) -> Vec<ProcessStatus> {
		let services = self.inner.services.lock().await;
		services
			.iter()
			.map(|(name, cell)| cell.status(name))
			.collect()
	}

	pub async fn status_of(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
		let services = self.inner.services.lock().await;
		services
			.get(name)
			.map(|cell| cell.status(name))
			.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))
	}

	pub async fn logs(&self, name: &str, lines: usize) -> Result<Vec<String>, SupervisorError> {
		let kind = {
			let services = self.inner.services.lock().await;
			services
				.get(name)
				.map(|cell| cell.kind.clone())
				.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?
		};
		match kind {
			ServiceKind::Compose { dir, .. } => compose_logs(&dir, &self.inner.containers, lines)
				.await
				.map_err(SupervisorError::Runtime),
			_ => self
				.inner
				.logs
				.tail(name, lines)
				.map_err(|e| SupervisorError::Runtime(e.to_string())),
		}
	}

	/// Adopt-first startup: anything already alive is left exactly as it is.
	/// A per-service failure marks that service failed and moves on. Begins
	/// the recurring health pass.
	pub async fn up(&self) {
		if let Err(err) = self.inner.logs.init() {
			warn!("log directory unavailable: {err}");
		}
		let names: Vec<String> = {
			let services = self.inner.services.lock().await;
			services.keys().cloned().collect()
		};
		for name in names {
			if let Err(err) = Inner::start_service(&self.inner, &name).await {
				let detail = err.to_string();
				error!("[{name}] failed to start: {detail}");
				Inner::update(&self.inner, &name, |cell| {
					cell.state = ProcessState::Failed;
					cell.detail = detail.clone();
				})
				.await;
			}
		}
		Inner::probe_all(&self.inner).await;

		let inner = self.inner.clone();
		let interval = inner.timing.health_interval;
		let task = tokio::spawn(async move {
			loop {
				sleep(interval).await;
				Inner::probe_all(&inner).await;
			}
		});
		if let Some(previous) = self.inner.health_task.lock().unwrap().replace(task) {
			previous.abort();
		}
	}

	/// Stop the health pass. Running services are left exactly as they are —
	/// supervision going away must not take the machine's services with it.
	pub fn shutdown(&self) {
		if let Some(task) = self.inner.health_task.lock().unwrap().take() {
			task.abort();
		}
	}

	pub async fn start(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
		{
			let mut services = self.inner.services.lock().await;
			let cell = services
				.get_mut(name)
				.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?;
			if matches!(cell.state, ProcessState::Running | ProcessState::Starting) {
				return Ok(cell.status(name));
			}
			cell.fast_exits = 0;
			cell.restarts = 0;
		}
		Inner::start_service(&self.inner, name).await?;
		Inner::probe_service(&self.inner, name).await;
		self.status_of(name).await
	}

	pub async fn stop(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
		let kind = {
			let services = self.inner.services.lock().await;
			services
				.get(name)
				.map(|cell| cell.kind.clone())
				.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?
		};
		match kind {
			ServiceKind::External { .. } => {
				return Err(SupervisorError::ObservedService(name.to_string()));
			}
			ServiceKind::Compose { dir, .. } => {
				compose_stop(&dir, &self.inner.containers)
					.await
					.map_err(SupervisorError::Runtime)?;
				Inner::update(&self.inner, name, |cell| cell.mark_stopped()).await;
			}
			ServiceKind::Daemon { .. } => {
				{
					let services = self.inner.services.lock().await;
					if services.get(name).and_then(|cell| cell.ownership)
						== Some(Ownership::Adopted)
					{
						return Err(SupervisorError::AdoptedService(name.to_string()));
					}
				}
				Inner::stop_owned(&self.inner, name).await;
			}
		}
		self.status_of(name).await
	}

	pub async fn restart(&self, name: &str) -> Result<ProcessStatus, SupervisorError> {
		let kind = {
			let services = self.inner.services.lock().await;
			services
				.get(name)
				.map(|cell| cell.kind.clone())
				.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?
		};
		match kind {
			ServiceKind::External { .. } => {
				return Err(SupervisorError::ObservedService(name.to_string()));
			}
			ServiceKind::Compose { dir, env, .. } => {
				Inner::ensure_containers(&self.inner).await?;
				compose_restart(&dir, &self.inner.containers, &env)
					.await
					.map_err(SupervisorError::Runtime)?;
				Inner::update(&self.inner, name, |cell| cell.since = Some(Utc::now())).await;
			}
			ServiceKind::Daemon { .. } => {
				let (adopted, has_child) = {
					let services = self.inner.services.lock().await;
					let cell = services
						.get(name)
						.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?;
					(
						cell.ownership == Some(Ownership::Adopted),
						cell.child.is_some(),
					)
				};
				if adopted {
					return Err(SupervisorError::AdoptedService(name.to_string()));
				}
				// The old child must exit first or its crash-restart logic
				// races the replacement.
				if has_child {
					Inner::stop_owned(&self.inner, name).await;
				}
				let spec = daemon_spawn_spec(&kind).map_err(SupervisorError::Runtime)?;
				Inner::spawn_owned(&self.inner, name, spec).await;
			}
		}
		Inner::probe_service(&self.inner, name).await;
		self.status_of(name).await
	}
}

fn daemon_spawn_spec(kind: &ServiceKind) -> Result<SpawnSpec, String> {
	match kind {
		ServiceKind::Daemon {
			command, cwd, env, ..
		} => Ok(SpawnSpec {
			cmd: command.clone(),
			cwd: cwd.clone(),
			env: env
				.iter()
				.map(|(key, value)| (key.clone(), Some(value.clone())))
				.collect(),
		}),
		ServiceKind::Compose { .. } => {
			Err("compose services are never spawned as child processes".to_string())
		}
		ServiceKind::External { .. } => Err("observed services are never spawned".to_string()),
	}
}

impl Inner {
	/// Apply a mutation to one service and broadcast its status when the
	/// mutation changed it.
	async fn update<R>(
		inner: &Arc<Inner>,
		name: &str,
		mutation: impl FnOnce(&mut ServiceCell) -> R,
	) -> Option<R> {
		let mut services = inner.services.lock().await;
		let cell = services.get_mut(name)?;
		let before = cell.status(name);
		let result = mutation(cell);
		let after = cell.status(name);
		if before != after {
			let _ = inner.status_tx.send(after);
		}
		Some(result)
	}

	async fn ensure_containers(inner: &Arc<Inner>) -> Result<(), SupervisorError> {
		let mut ready = inner.containers_ready.lock().await;
		if *ready {
			return Ok(());
		}
		let status = ensure_container_runtime(&inner.containers)
			.await
			.map_err(SupervisorError::Runtime)?;
		if !status.reachable {
			return Err(SupervisorError::Runtime(format!(
				"container runtime unreachable: {}",
				status.detail
			)));
		}
		*ready = true;
		Ok(())
	}

	async fn start_service(inner: &Arc<Inner>, name: &str) -> Result<(), SupervisorError> {
		let kind = Inner::update(inner, name, |cell| {
			cell.stop_requested = false;
			cell.kind.clone()
		})
		.await
		.ok_or_else(|| SupervisorError::UnknownService(name.to_string()))?;

		match kind {
			ServiceKind::Compose { dir, env, .. } => {
				Inner::ensure_containers(inner).await?;
				compose_up(&dir, &inner.containers, &env)
					.await
					.map_err(SupervisorError::Runtime)?;
				Inner::update(inner, name, |cell| {
					cell.ownership = Some(Ownership::Compose);
					cell.state = ProcessState::Running;
					cell.since = Some(Utc::now());
				})
				.await;
			}
			ServiceKind::Daemon { ref health, .. } => {
				let probe = probe_http(health, None).await;
				if probe.ok {
					info!(
						"[{name}] adopted already-running service ({})",
						probe.detail
					);
					Inner::update(inner, name, |cell| cell.adopt(None, probe.detail.clone())).await;
				} else {
					let spec = daemon_spawn_spec(&kind).map_err(SupervisorError::Runtime)?;
					Inner::spawn_owned(inner, name, spec).await;
				}
			}
			ServiceKind::External { .. } => {
				Inner::probe_observed(inner, name).await;
			}
		}
		Ok(())
	}

	// Boxed to break the async cycle: a spawned child's exit handler can
	// schedule another spawn.
	fn spawn_owned<'a>(
		inner: &'a Arc<Inner>,
		name: &'a str,
		spec: SpawnSpec,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
		Box::pin(Self::spawn_owned_inner(inner, name, spec))
	}

	async fn spawn_owned_inner(inner: &Arc<Inner>, name: &str, spec: SpawnSpec) {
		let spawn_failed = |detail: String| {
			let inner = inner.clone();
			let name = name.to_string();
			async move {
				error!("[{name}] {detail}");
				Inner::update(&inner, &name, |cell| {
					cell.state = ProcessState::Failed;
					cell.health = HealthState::Unknown;
					cell.detail = detail.clone();
				})
				.await;
			}
		};

		if let Err(err) = inner.logs.init() {
			spawn_failed(format!("spawn failed: {err}")).await;
			return;
		}
		let log_file = match inner.logs.open_append(name) {
			Ok(file) => file,
			Err(err) => {
				spawn_failed(format!("spawn failed: {err}")).await;
				return;
			}
		};
		let stderr_file = match log_file.try_clone() {
			Ok(file) => file,
			Err(err) => {
				spawn_failed(format!("spawn failed: {err}")).await;
				return;
			}
		};

		let Some((program, args)) = spec.cmd.split_first() else {
			spawn_failed("spawn failed: empty command".to_string()).await;
			return;
		};
		let mut command = Command::new(program);
		command
			.args(args)
			.current_dir(&spec.cwd)
			.stdin(Stdio::null())
			.stdout(Stdio::from(log_file))
			.stderr(Stdio::from(stderr_file));
		for (key, value) in &spec.env {
			match value {
				Some(value) => {
					command.env(key, value);
				}
				None => {
					command.env_remove(key);
				}
			}
		}

		let mut child = match command.spawn() {
			Ok(child) => child,
			Err(err) => {
				spawn_failed(format!("spawn failed: {err}")).await;
				return;
			}
		};
		let pid = child.id().unwrap_or_default();
		let (exited_tx, exited_rx) = watch::channel(false);
		let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<KillSignal>();

		let generation = Inner::update(inner, name, |cell| {
			cell.generation += 1;
			// A fresh spawn supersedes any earlier stop request; the new
			// child's exits are crashes.
			cell.stop_requested = false;
			cell.child = Some(ChildHandle {
				exited: exited_rx,
				kill: kill_tx,
			});
			cell.pid = Some(pid);
			cell.ownership = Some(Ownership::Owned);
			cell.state = ProcessState::Starting;
			cell.since = Some(Utc::now());
			cell.spawned_at = Some(Instant::now());
			cell.detail = format!("spawned pid {pid}");
			cell.generation
		})
		.await;
		let Some(generation) = generation else {
			// The service was removed while spawning; nothing owns the child.
			let _ = child.start_kill();
			return;
		};
		info!("[{name}] spawned {} (pid {pid})", spec.cmd.join(" "));

		let monitor_inner = inner.clone();
		let monitor_name = name.to_string();
		tokio::spawn(async move {
			let code = loop {
				tokio::select! {
					status = child.wait() => {
						break status.ok().and_then(|s| s.code()).unwrap_or(-1);
					}
					signal = kill_rx.recv() => match signal {
						Some(KillSignal::Term) => terminate(pid, &mut child),
						Some(KillSignal::Kill) => {
							let _ = child.start_kill();
						}
						None => {}
					},
				}
			};
			let _ = exited_tx.send(true);
			Inner::on_child_exit(&monitor_inner, &monitor_name, generation, code).await;
		});
	}

	async fn on_child_exit(inner: &Arc<Inner>, name: &str, generation: u64, code: i32) {
		let timing = inner.timing.clone();
		enum Next {
			Done,
			Respawn(std::time::Duration),
		}
		let next = Inner::update(inner, name, |cell| {
			if cell.generation != generation {
				return Next::Done;
			}
			cell.child = None;
			cell.pid = None;
			if cell.stop_requested {
				cell.mark_stopped();
				info!("[{name}] stopped (exit code {code})");
				return Next::Done;
			}
			// A dev server's lifecycle belongs to the developer: any exit — a
			// manual kill, a package install invalidating the process — is a
			// stop, never a crash to converge on.
			if cell.kind.is_dev() {
				cell.mark_stopped();
				cell.detail = format!("exited (code {code}); dev servers are not respawned");
				info!("[{name}] {}", cell.detail);
				return Next::Done;
			}
			let ran = cell.spawned_at.map(|at| at.elapsed()).unwrap_or_default();
			cell.fast_exits = if ran < timing.fast_exit {
				cell.fast_exits + 1
			} else {
				0
			};
			if cell.fast_exits >= timing.max_fast_exits {
				cell.state = ProcessState::Failed;
				cell.health = HealthState::Unknown;
				cell.detail = format!(
					"gave up after {} fast exits (last code {code})",
					timing.max_fast_exits
				);
				error!("[{name}] {}", cell.detail);
				return Next::Done;
			}
			cell.restarts += 1;
			let backoff = timing.backoff(cell.fast_exits);
			cell.state = ProcessState::Starting;
			cell.detail = format!(
				"exited (code {code}) after {}s; restarting in {}ms",
				ran.as_secs_f64().round() as u64,
				backoff.as_millis()
			);
			warn!("[{name}] {}", cell.detail);
			Next::Respawn(backoff)
		})
		.await
		.unwrap_or(Next::Done);

		if let Next::Respawn(backoff) = next {
			let respawn_inner = inner.clone();
			let respawn_name = name.to_string();
			let task = tokio::spawn(async move {
				sleep(backoff).await;
				Inner::update(&respawn_inner, &respawn_name, |cell| {
					cell.restart_task = None;
				})
				.await;
				Inner::respawn(&respawn_inner, &respawn_name).await;
			});
			Inner::update(inner, name, |cell| {
				cell.restart_task = Some(task);
			})
			.await;
		}
	}

	/// Spec construction can itself fail; that failure must land on the
	/// service, not escape a background task.
	async fn respawn(inner: &Arc<Inner>, name: &str) {
		let spec = {
			let services = inner.services.lock().await;
			let Some(cell) = services.get(name) else {
				return;
			};
			daemon_spawn_spec(&cell.kind)
		};
		match spec {
			Ok(spec) => Inner::spawn_owned(inner, name, spec).await,
			Err(err) => {
				let detail = format!("spawn spec failed: {err}");
				error!("[{name}] {detail}");
				Inner::update(inner, name, |cell| {
					cell.state = ProcessState::Failed;
					cell.health = HealthState::Unknown;
					cell.detail = detail.clone();
				})
				.await;
			}
		}
	}

	async fn stop_owned(inner: &Arc<Inner>, name: &str) {
		let handle = {
			let mut services = inner.services.lock().await;
			let Some(cell) = services.get_mut(name) else {
				return;
			};
			if let Some(task) = cell.restart_task.take() {
				task.abort();
			}
			match &cell.child {
				Some(child) => {
					cell.stop_requested = true;
					Some((child.kill.clone(), child.exited.clone()))
				}
				None => None,
			}
		};
		let Some((kill, mut exited)) = handle else {
			Inner::update(inner, name, |cell| cell.mark_stopped()).await;
			return;
		};
		let _ = kill.send(KillSignal::Term);
		if !*exited.borrow() {
			tokio::select! {
				_ = exited.changed() => {}
				_ = sleep(inner.timing.stop_grace) => {
					let _ = kill.send(KillSignal::Kill);
					let _ = exited.changed().await;
				}
			}
		}
	}

	async fn probe_all(inner: &Arc<Inner>) {
		let names: Vec<String> = {
			let services = inner.services.lock().await;
			services
				.iter()
				.filter(|(_, cell)| {
					// Observed services are always re-probed — a dead one can
					// come back without any action of ours. Spawnable ones
					// stay down until started.
					cell.kind.is_observed()
						|| !matches!(cell.state, ProcessState::Stopped | ProcessState::Failed)
				})
				.map(|(name, _)| name.clone())
				.collect()
		};
		for name in names {
			Inner::probe_service(inner, &name).await;
		}
		Inner::rotate_logs(inner).await;
	}

	// Rotation rides the health cadence rather than a timer of its own: a log
	// only grows while its writer runs, which is the same thing the health
	// pass is already walking.
	async fn rotate_logs(inner: &Arc<Inner>) {
		let names: Vec<String> = {
			let services = inner.services.lock().await;
			services.keys().cloned().collect()
		};
		for name in names {
			match inner.logs.rotate(&name) {
				Ok(true) => info!("[{name}] log reached the size cap; rotated to .1"),
				Ok(false) => {}
				Err(err) => error!("[{name}] log rotation failed: {err}"),
			}
		}
	}

	// Observed services: probe, display, embed — never spawn. A failed probe
	// marks the service failed but probing continues, since a remote machine
	// can come back on its own.
	async fn probe_observed(inner: &Arc<Inner>, name: &str) {
		let health = Inner::update(inner, name, |cell| {
			cell.ownership = Some(Ownership::External);
			match &cell.kind {
				ServiceKind::External { health } => health.clone(),
				_ => None,
			}
		})
		.await
		.flatten();

		let Some(health) = health else {
			Inner::update(inner, name, |cell| {
				cell.state = ProcessState::Running;
				cell.health = HealthState::Unknown;
				cell.detail = "no health check".to_string();
			})
			.await;
			return;
		};
		let probe = probe_http(&health, None).await;
		Inner::update(inner, name, |cell| {
			cell.detail = probe.detail.clone();
			if probe.ok {
				if cell.state != ProcessState::Running {
					cell.since = Some(Utc::now());
				}
				cell.state = ProcessState::Running;
				cell.health = HealthState::Ok;
			} else {
				cell.state = ProcessState::Failed;
				cell.health = HealthState::Failing;
				cell.since = None;
			}
		})
		.await;
	}

	async fn probe_service(inner: &Arc<Inner>, name: &str) {
		let kind = {
			let services = inner.services.lock().await;
			let Some(cell) = services.get(name) else {
				return;
			};
			cell.kind.clone()
		};
		let health = match &kind {
			ServiceKind::External { .. } => {
				Inner::probe_observed(inner, name).await;
				return;
			}
			ServiceKind::Daemon { health, .. } | ServiceKind::Compose { health, .. } => {
				health.clone()
			}
		};

		let probe: ProbeResult = probe_http(&health, None).await;
		let timing = inner.timing.clone();
		let take_ownership = Inner::update(inner, name, |cell| {
			cell.detail = probe.detail.clone();
			cell.health = if probe.ok {
				HealthState::Ok
			} else {
				HealthState::Failing
			};
			if probe.ok {
				cell.probe_failures = 0;
				// For an owned service the probe only counts while our child
				// is alive — a healthy endpoint can belong to a process
				// someone else started (e.g. a launcher that detached its
				// real server and exited).
				let child_alive = cell.child.is_some();
				if cell.ownership != Some(Ownership::Owned) || child_alive {
					if cell.state == ProcessState::Starting {
						cell.state = ProcessState::Running;
					}
					if child_alive
						&& cell
							.spawned_at
							.is_some_and(|at| at.elapsed() > timing.fast_exit)
					{
						cell.fast_exits = 0;
					}
				}
				return false;
			}
			// Adopted services have no child handle, so death is only visible
			// through failed probes. Persistent failure converges them to
			// owned children.
			if cell.ownership == Some(Ownership::Adopted) {
				cell.probe_failures += 1;
				if cell.probe_failures >= timing.adopted_death_threshold {
					cell.probe_failures = 0;
					if cell.kind.is_dev() {
						cell.mark_stopped();
						cell.detail = "dev server went away; not respawned".to_string();
						info!("[{name}] {}", cell.detail);
						return false;
					}
					warn!(
						"[{name}] adopted service unhealthy for {} probes; taking ownership",
						timing.adopted_death_threshold
					);
					return true;
				}
			}
			false
		})
		.await
		.unwrap_or(false);

		if take_ownership {
			Inner::respawn(inner, name).await;
		}
	}
}

#[cfg(unix)]
fn terminate(pid: u32, _child: &mut tokio::process::Child) {
	unsafe {
		libc::kill(pid as i32, libc::SIGTERM);
	}
}

#[cfg(not(unix))]
fn terminate(_pid: u32, child: &mut tokio::process::Child) {
	let _ = child.start_kill();
}
