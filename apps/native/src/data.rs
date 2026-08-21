//! The async data plane between the daemon and the UI.
//!
//! The frame loop never awaits: all daemon traffic happens on a dedicated
//! tokio runtime running on background threads, and the UI reads immutable
//! snapshots out of watch channels. [`DataHandle`] is the whole surface the
//! views see — snapshot reads, watch receivers for wakeups, and fire-and-forget
//! commands.
//!
//! The plane owns one [`CoreClient`] for queries, one [`SubscriptionBroker`]
//! for events, and one [`LibraryContext`] carrying the persisted library
//! selection. Refreshes are driven by daemon events through [`RefreshGate`]s:
//! events are invalidation hints (the broker can replay duplicates across
//! reconnects), so every refresh is an idempotent refetch, coalesced so an
//! event storm costs one query per debounce window, never one per event.
//!
//! Daemon liveness is a ping on an interval — the same cadence the old
//! monitor thread used, feeding the same status dot. The plane never spawns
//! the daemon; offline it simply publishes offline snapshots and keeps
//! retrying.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use uuid::Uuid;

use sd_client::{
	daemon_socket_addr, is_daemon_running, BrokerSubscription, CoreClient, EventFilter,
	LibraryContext, SubscriptionBroker,
};
use sd_core::domain::file::{EntryKind, File};
use sd_core::domain::SdPath;
use sd_core::ops::core::status::output::CoreStatus;
use sd_core::ops::files::query::{
	DirectoryListingInput, DirectoryListingOutput, DirectorySortBy,
};
use sd_core::ops::volumes::{VolumeFilter, VolumeListOutput, VolumeListQueryInput};

/// How often the plane pings the daemon for liveness.
const PING_INTERVAL: Duration = Duration::from_secs(3);

/// How long an invalidation hint waits before the refetch fires. The deadline
/// is set by the first hint and later hints coalesce into it, so staleness is
/// bounded even under a continuous event stream.
const REFRESH_DEBOUNCE: Duration = Duration::from_millis(250);

/// Commands the UI sends into the plane. All fire-and-forget.
enum Command {
	OpenDirectory(PathBuf),
	SelectLibrary(Uuid),
}

/// One library the daemon reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRow {
	pub id: Uuid,
	pub name: String,
}

/// One volume the daemon reports, pre-formatted for the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeRow {
	pub id: Uuid,
	pub name: String,
	pub mount_point: PathBuf,
	/// Total capacity, human formatted; empty when the daemon reports zero.
	pub capacity_label: String,
}

/// Sidebar state: connection, libraries, volumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SidebarSnapshot {
	pub online: bool,
	pub libraries: Vec<LibraryRow>,
	pub current_library: Option<Uuid>,
	pub volumes: Vec<VolumeRow>,
}

/// One row of the explorer list, pre-formatted so the frame loop only draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
	pub name: String,
	pub is_dir: bool,
	pub size_label: String,
	pub modified_label: String,
	/// Local path, for navigating into directories.
	pub path: PathBuf,
}

/// Where the current listing stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingPhase {
	/// Nothing has been opened yet.
	Idle,
	/// A fetch for the current target is outstanding.
	Loading,
	/// Rows are current for the target.
	Ready,
	/// The daemon is not answering; rows (if any) are stale.
	Offline,
	/// The daemon is up but no library exists to route the query through.
	NoLibrary,
	Error(String),
}

/// The explorer listing state for the current target directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingSnapshot {
	pub target: Option<PathBuf>,
	pub phase: ListingPhase,
	pub rows: Arc<Vec<FileRow>>,
}

impl Default for ListingSnapshot {
	fn default() -> Self {
		ListingSnapshot {
			target: None,
			phase: ListingPhase::Idle,
			rows: Arc::new(Vec::new()),
		}
	}
}

/// The UI's handle to the data plane. Cloning is cheap; all clones share the
/// same plane.
#[derive(Clone)]
pub struct DataHandle {
	socket_addr: String,
	commands: mpsc::UnboundedSender<Command>,
	sidebar: watch::Receiver<Arc<SidebarSnapshot>>,
	listing: watch::Receiver<Arc<ListingSnapshot>>,
}

impl DataHandle {
	/// Current sidebar snapshot. Non-blocking; safe from the frame loop.
	pub fn sidebar(&self) -> Arc<SidebarSnapshot> {
		self.sidebar.borrow().clone()
	}

	/// Current listing snapshot. Non-blocking; safe from the frame loop.
	pub fn listing(&self) -> Arc<ListingSnapshot> {
		self.listing.borrow().clone()
	}

	/// Watch for sidebar changes. `changed()` is runtime-agnostic, so UI
	/// tasks can await it on gpui's executor.
	pub fn watch_sidebar(&self) -> watch::Receiver<Arc<SidebarSnapshot>> {
		self.sidebar.clone()
	}

	/// Watch for listing changes.
	pub fn watch_listing(&self) -> watch::Receiver<Arc<ListingSnapshot>> {
		self.listing.clone()
	}

	/// The daemon socket address the plane talks to, for display.
	pub fn socket_addr(&self) -> &str {
		&self.socket_addr
	}

	/// Load `path` into the explorer listing.
	pub fn open_directory(&self, path: PathBuf) {
		let _ = self.commands.send(Command::OpenDirectory(path));
	}

	/// Switch the current library.
	pub fn select_library(&self, id: Uuid) {
		let _ = self.commands.send(Command::SelectLibrary(id));
	}
}

/// Start the data plane on its own runtime and return the UI handle.
///
/// `instance` selects a named daemon instance (its own port and data
/// directory); `None` targets the default daemon.
pub fn spawn(instance: Option<String>) -> DataHandle {
	let socket_addr = daemon_socket_addr(instance.as_deref()).to_string();
	let (commands_tx, commands_rx) = mpsc::unbounded_channel();
	let (sidebar_tx, sidebar_rx) = watch::channel(Arc::new(SidebarSnapshot::default()));
	let (listing_tx, listing_rx) = watch::channel(Arc::new(ListingSnapshot::default()));

	let addr = socket_addr.clone();
	std::thread::Builder::new()
		.name("sd-data".into())
		.spawn(move || {
			let runtime = tokio::runtime::Builder::new_multi_thread()
				.worker_threads(2)
				.thread_name("sd-data-worker")
				.enable_all()
				.build()
				.expect("failed to build data-plane runtime");
			runtime.block_on(run(addr, instance, commands_rx, sidebar_tx, listing_tx));
		})
		.expect("failed to spawn data-plane thread");

	DataHandle {
		socket_addr,
		commands: commands_tx,
		sidebar: sidebar_rx,
		listing: listing_rx,
	}
}

/// Where the persisted library selection lives: inside the daemon instance's
/// data directory, so named test instances never touch the default state.
fn library_state_path(instance: Option<&str>) -> PathBuf {
	let base = sd_core::config::default_data_dir()
		.unwrap_or_else(|_| std::env::temp_dir().join("spacedrive"));
	let base = sd_core::infra::daemon::addr::instance_data_dir(base, instance);
	base.join("native-app").join("library-state.json")
}

/// Everything a completed background task reports into the main loop. Event
/// pumps also land here, so the loop is a single-consumer state machine with
/// no shared mutability.
enum TaskResult {
	Ping(bool),
	SidebarFetched(anyhow::Result<SidebarFetch>),
	ListingFetched {
		generation: u64,
		result: anyhow::Result<Vec<FileRow>>,
	},
	SidebarEvent,
	ListingEvent { generation: u64 },
}

struct SidebarFetch {
	device_slug: String,
	libraries: Vec<LibraryRow>,
	current_library: Option<Uuid>,
	volumes: Vec<VolumeRow>,
}

async fn run(
	socket_addr: String,
	instance: Option<String>,
	mut commands: mpsc::UnboundedReceiver<Command>,
	sidebar_tx: watch::Sender<Arc<SidebarSnapshot>>,
	listing_tx: watch::Sender<Arc<ListingSnapshot>>,
) {
	let client = CoreClient::new(socket_addr.clone());
	let library = load_library_context(&client, instance.as_deref()).await;
	let broker = SubscriptionBroker::new(socket_addr);
	let (results_tx, mut results) = mpsc::unbounded_channel();

	// Sidebar invalidation: volume/library lifecycle variants plus normalized
	// volume resource events. Both pumps live for the app's lifetime; the
	// broker reconnects them on daemon restarts.
	let volume_events = broker.subscribe(
		[
			"VolumeAdded",
			"VolumeRemoved",
			"VolumeUpdated",
			"VolumeMountChanged",
			"LibraryCreated",
			"LibraryOpened",
			"LibraryClosed",
			"LibraryDeleted",
		]
		.into_iter()
		.map(String::from)
		.collect(),
		None,
	);
	let volume_resources = broker.subscribe(
		resource_event_types(),
		Some(resource_filter("volume", None, None)),
	);
	pump(volume_events, results_tx.clone(), || TaskResult::SidebarEvent);
	pump(volume_resources, results_tx.clone(), || TaskResult::SidebarEvent);

	let mut plane = Plane {
		client,
		broker,
		library,
		results_tx,
		sidebar_tx,
		listing_tx,
		online: false,
		device_slug: None,
		libraries: Vec::new(),
		current_library: None,
		volumes: Vec::new(),
		target: None,
		target_generation: 0,
		rows: Arc::new(Vec::new()),
		phase: ListingPhase::Idle,
		sidebar_gate: RefreshGate::new(REFRESH_DEBOUNCE),
		listing_gate: RefreshGate::new(REFRESH_DEBOUNCE),
		listing_pump: None,
	};

	let mut ping = tokio::time::interval(PING_INTERVAL);
	ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

	loop {
		let sidebar_deadline = plane.sidebar_gate.deadline();
		let listing_deadline = plane.listing_gate.deadline();
		tokio::select! {
			command = commands.recv() => match command {
				Some(command) => plane.handle_command(command),
				// The UI dropped its handle; the app is going down.
				None => break,
			},
			result = results.recv() => match result {
				Some(result) => plane.handle_result(result),
				None => break,
			},
			_ = ping.tick() => plane.spawn_ping(),
			_ = sleep_until_or_never(sidebar_deadline) => plane.fire_sidebar(),
			_ = sleep_until_or_never(listing_deadline) => plane.fire_listing(),
		}
	}
}

/// Load the persisted library selection, recovering from a corrupt state file
/// by discarding it — the selection is a preference, never data.
async fn load_library_context(client: &CoreClient, instance: Option<&str>) -> LibraryContext {
	let path = library_state_path(instance);
	match LibraryContext::load(client.clone(), &path).await {
		Ok(context) => context,
		Err(error) => {
			eprintln!(
				"spacedrive-native: discarding unreadable library state at {}: {error:#}",
				path.display()
			);
			let _ = tokio::fs::remove_file(&path).await;
			LibraryContext::load(client.clone(), &path)
				.await
				.expect("library context loads once the state file is gone")
		}
	}
}

/// Await a deadline, or never complete when there is none. Lets the select
/// loop treat "no refresh owed" as an arm that simply never fires.
async fn sleep_until_or_never(deadline: Option<Instant>) {
	match deadline {
		Some(deadline) => tokio::time::sleep_until(deadline).await,
		None => std::future::pending().await,
	}
}

/// Forward every event on `subscription` into the results channel as the
/// message `make` builds. Ends when the subscription or the channel closes.
fn pump(
	mut subscription: BrokerSubscription,
	results: mpsc::UnboundedSender<TaskResult>,
	make: impl Fn() -> TaskResult + Send + 'static,
) -> JoinHandle<()> {
	tokio::spawn(async move {
		while subscription.recv().await.is_some() {
			if results.send(make()).is_err() {
				break;
			}
		}
	})
}

fn resource_event_types() -> Vec<String> {
	["ResourceChanged", "ResourceChangedBatch", "ResourceDeleted"]
		.into_iter()
		.map(String::from)
		.collect()
}

/// A resource-scoped event filter; adds an exact-match path scope when the
/// scope is known, mirroring the web explorer's per-directory subscriptions.
fn resource_filter(
	resource_type: &str,
	library_id: Option<Uuid>,
	path_scope: Option<SdPath>,
) -> EventFilter {
	let include_descendants = path_scope.is_some().then_some(false);
	EventFilter {
		library_id,
		job_id: None,
		device_id: None,
		resource_type: Some(resource_type.to_string()),
		path_scope,
		include_descendants,
	}
}

/// The single-threaded state machine behind the select loop.
struct Plane {
	client: CoreClient,
	broker: SubscriptionBroker,
	library: LibraryContext,
	results_tx: mpsc::UnboundedSender<TaskResult>,
	sidebar_tx: watch::Sender<Arc<SidebarSnapshot>>,
	listing_tx: watch::Sender<Arc<ListingSnapshot>>,

	online: bool,
	/// The daemon's device slug, learned from `core.status`; physical SdPaths
	/// sent back to the daemon carry it so both sides agree on addressing.
	device_slug: Option<String>,
	libraries: Vec<LibraryRow>,
	current_library: Option<Uuid>,
	volumes: Vec<VolumeRow>,

	target: Option<PathBuf>,
	/// Bumped on every target change; results and events tagged with an older
	/// generation are ignored.
	target_generation: u64,
	rows: Arc<Vec<FileRow>>,
	phase: ListingPhase,

	sidebar_gate: RefreshGate,
	listing_gate: RefreshGate,
	listing_pump: Option<JoinHandle<()>>,
}

impl Plane {
	fn handle_command(&mut self, command: Command) {
		match command {
			Command::OpenDirectory(path) => {
				if self.target.as_deref() == Some(&path) {
					// Same directory re-opened: refresh, keep current rows.
					self.listing_gate.mark_immediate(Instant::now());
					return;
				}
				self.target = Some(path);
				self.target_generation += 1;
				self.rows = Arc::new(Vec::new());
				self.phase = ListingPhase::Loading;
				self.publish_listing();
				self.resubscribe_listing();
				self.listing_gate.mark_immediate(Instant::now());
			}
			Command::SelectLibrary(id) => {
				let library = self.library.clone();
				let results = self.results_tx.clone();
				let generation = self.target_generation;
				tokio::spawn(async move {
					if let Err(error) = library.set_current(Some(id)).await {
						eprintln!("spacedrive-native: library selection not persisted: {error:#}");
					}
					// Refetch only after the selection is applied, so the
					// listing routes through the new library, not the old one.
					let _ = results.send(TaskResult::SidebarEvent);
					let _ = results.send(TaskResult::ListingEvent { generation });
				});
				self.current_library = Some(id);
				self.publish_sidebar();
				self.resubscribe_listing();
			}
		}
	}

	fn handle_result(&mut self, result: TaskResult) {
		match result {
			TaskResult::Ping(alive) => self.handle_ping(alive),
			TaskResult::SidebarFetched(result) => self.handle_sidebar_fetched(result),
			TaskResult::ListingFetched { generation, result } => {
				self.handle_listing_fetched(generation, result)
			}
			TaskResult::SidebarEvent => self.sidebar_gate.mark(Instant::now()),
			TaskResult::ListingEvent { generation } => {
				if generation == self.target_generation {
					self.listing_gate.mark(Instant::now());
				}
			}
		}
	}

	fn handle_ping(&mut self, alive: bool) {
		if alive == self.online {
			return;
		}
		self.online = alive;
		let now = Instant::now();
		if alive {
			self.sidebar_gate.mark_immediate(now);
			if self.target.is_some() {
				self.listing_gate.mark_immediate(now);
			}
		} else if self.target.is_some() {
			self.phase = ListingPhase::Offline;
			self.publish_listing();
		}
		self.publish_sidebar();
	}

	fn handle_sidebar_fetched(&mut self, result: anyhow::Result<SidebarFetch>) {
		// A rerun requested mid-fetch re-armed the deadline; the select loop
		// picks it up on its next pass.
		self.sidebar_gate.complete(Instant::now());
		match result {
			Ok(fetch) => {
				let slug_learned = self.device_slug.as_deref() != Some(&fetch.device_slug);
				let library_changed = self.current_library != fetch.current_library;
				self.online = true;
				self.device_slug = Some(fetch.device_slug);
				self.libraries = fetch.libraries;
				self.current_library = fetch.current_library;
				self.volumes = fetch.volumes;
				self.publish_sidebar();
				if (slug_learned || library_changed) && self.target.is_some() {
					// The listing could not be scoped (or routed) before; it
					// can now.
					self.resubscribe_listing();
					self.listing_gate.mark_immediate(Instant::now());
				}
			}
			Err(error) => {
				// The ping loop owns the offline transition; a failed fetch
				// alone (e.g. mid-shutdown) just logs.
				eprintln!("spacedrive-native: sidebar refresh failed: {error:#}");
			}
		}
	}

	fn handle_listing_fetched(&mut self, generation: u64, result: anyhow::Result<Vec<FileRow>>) {
		self.listing_gate.complete(Instant::now());
		if generation != self.target_generation {
			return;
		}
		match result {
			Ok(rows) => {
				self.rows = Arc::new(rows);
				self.phase = ListingPhase::Ready;
			}
			Err(error) => {
				self.phase = ListingPhase::Error(format!("{error:#}"));
			}
		}
		self.publish_listing();
	}

	fn spawn_ping(&self) {
		let client = self.client.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let _ = results.send(TaskResult::Ping(is_daemon_running(&client).await));
		});
	}

	fn fire_sidebar(&mut self) {
		if !self.sidebar_gate.fire() {
			return;
		}
		let client = self.client.clone();
		let library = self.library.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let result = fetch_sidebar(client, library).await;
			let _ = results.send(TaskResult::SidebarFetched(result));
		});
	}

	fn fire_listing(&mut self) {
		if !self.listing_gate.fire() {
			return;
		}
		let Some(target) = self.target.clone() else {
			self.listing_gate.complete(Instant::now());
			return;
		};
		if !self.online {
			self.phase = ListingPhase::Offline;
			self.publish_listing();
			self.listing_gate.complete(Instant::now());
			return;
		}
		let Some(device_slug) = self.device_slug.clone() else {
			// Online but the slug is not known yet; the sidebar fetch marks
			// the gate again once it lands.
			self.listing_gate.complete(Instant::now());
			return;
		};
		if self.current_library.is_none() {
			self.phase = ListingPhase::NoLibrary;
			self.publish_listing();
			self.listing_gate.complete(Instant::now());
			return;
		}
		let library = self.library.clone();
		let results = self.results_tx.clone();
		let generation = self.target_generation;
		tokio::spawn(async move {
			let result = fetch_listing(library, device_slug, target).await;
			let _ = results.send(TaskResult::ListingFetched { generation, result });
		});
	}

	/// Point the listing event subscription at the current target: scoped to
	/// the directory (exact-match, like the web explorer) when the device slug
	/// is known, broad within the `file` resource otherwise.
	fn resubscribe_listing(&mut self) {
		if let Some(pump_task) = self.listing_pump.take() {
			pump_task.abort();
		}
		let Some(target) = self.target.clone() else {
			return;
		};
		let path_scope = self.device_slug.clone().map(|device_slug| SdPath::Physical {
			device_slug,
			path: target,
		});
		let subscription = self.broker.subscribe(
			resource_event_types(),
			Some(resource_filter("file", self.current_library, path_scope)),
		);
		let generation = self.target_generation;
		self.listing_pump = Some(pump(subscription, self.results_tx.clone(), move || {
			TaskResult::ListingEvent { generation }
		}));
	}

	fn publish_sidebar(&self) {
		let _ = self.sidebar_tx.send(Arc::new(SidebarSnapshot {
			online: self.online,
			libraries: self.libraries.clone(),
			current_library: self.current_library,
			volumes: self.volumes.clone(),
		}));
	}

	fn publish_listing(&self) {
		let _ = self.listing_tx.send(Arc::new(ListingSnapshot {
			target: self.target.clone(),
			phase: self.phase.clone(),
			rows: self.rows.clone(),
		}));
	}
}

/// One sidebar refresh: `core.status` for device identity and libraries,
/// then `volumes.list` through the current library. Also reconciles the
/// persisted library selection against what actually exists — the first
/// library is adopted when nothing valid is selected.
async fn fetch_sidebar(client: CoreClient, library: LibraryContext) -> anyhow::Result<SidebarFetch> {
	let status: CoreStatus = client.query(&(), None).await?;

	let libraries: Vec<LibraryRow> = status
		.libraries
		.iter()
		.map(|info| LibraryRow {
			id: info.id,
			name: info.name.clone(),
		})
		.collect();

	let selected = library
		.current()
		.filter(|id| libraries.iter().any(|row| row.id == *id))
		.or_else(|| libraries.first().map(|row| row.id));
	if selected != library.current() {
		if let Err(error) = library.set_current(selected).await {
			eprintln!("spacedrive-native: library selection not persisted: {error:#}");
		}
	}

	let volumes = if selected.is_some() {
		match library
			.query::<_, VolumeListOutput>(&VolumeListQueryInput {
				filter: VolumeFilter::All,
			})
			.await
		{
			Ok(output) => output
				.volumes
				.into_iter()
				.map(|volume| VolumeRow {
					id: volume.id,
					name: volume.name,
					mount_point: volume.mount_point,
					capacity_label: if volume.total_capacity == 0 {
						String::new()
					} else {
						format_bytes(volume.total_capacity)
					},
				})
				.collect(),
			Err(error) => {
				eprintln!("spacedrive-native: volume listing failed: {error:#}");
				Vec::new()
			}
		}
	} else {
		Vec::new()
	};

	Ok(SidebarFetch {
		device_slug: status.device_info.slug,
		libraries,
		current_library: selected,
		volumes,
	})
}

/// One listing refresh: `files.directory_listing` for the target directory,
/// name-sorted with folders first, hidden files excluded — the same shape the
/// web explorer requests. Unindexed paths fall through to the daemon's
/// ephemeral indexer, so any local directory lists without setup.
async fn fetch_listing(
	library: LibraryContext,
	device_slug: String,
	target: PathBuf,
) -> anyhow::Result<Vec<FileRow>> {
	let input = DirectoryListingInput {
		path: SdPath::Physical {
			device_slug,
			path: target.clone(),
		},
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
		folders_first: Some(true),
	};
	let output: DirectoryListingOutput = library.query(&input).await?;
	Ok(output
		.files
		.iter()
		.map(|file| FileRow::from_file(file, &target))
		.collect())
}

impl FileRow {
	fn from_file(file: &File, parent: &std::path::Path) -> Self {
		let is_dir = matches!(file.kind, EntryKind::Directory);
		// Entry names are stored without their extension; display joins the
		// two, the same way the web explorer's title does.
		let name = match &file.extension {
			Some(extension) if !extension.is_empty() => format!("{}.{extension}", file.name),
			_ => file.name.clone(),
		};
		let path = match &file.sd_path {
			SdPath::Physical { path, .. } => path.clone(),
			_ => parent.join(&name),
		};
		FileRow {
			name,
			is_dir,
			size_label: if is_dir {
				"—".to_string()
			} else {
				format_bytes(file.size)
			},
			modified_label: file
				.modified_at
				.with_timezone(&chrono::Local)
				.format("%b %e, %Y %H:%M")
				.to_string(),
			path,
		}
	}
}

/// Human-readable byte count in decimal units, the convention storage
/// vendors and the web app's `formatBytes` share.
pub fn format_bytes(bytes: u64) -> String {
	const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
	let mut value = bytes as f64;
	let mut unit = 0;
	while value >= 1000.0 && unit < UNITS.len() - 1 {
		value /= 1000.0;
		unit += 1;
	}
	if unit == 0 {
		format!("{bytes} B")
	} else {
		format!("{value:.1} {}", UNITS[unit])
	}
}

/// Coalesces invalidation hints into at most one in-flight refetch.
///
/// Rules: the first hint arms a debounce deadline; further hints while armed
/// coalesce into it (the deadline never slides, so staleness stays bounded
/// under a steady event stream). A hint arriving while a fetch is in flight
/// requests a rerun, which arms immediately when the fetch completes. Target
/// changes use [`RefreshGate::mark_immediate`] to skip the debounce.
struct RefreshGate {
	debounce: Duration,
	deadline: Option<Instant>,
	in_flight: bool,
	rerun: bool,
}

impl RefreshGate {
	fn new(debounce: Duration) -> Self {
		RefreshGate {
			debounce,
			deadline: None,
			in_flight: false,
			rerun: false,
		}
	}

	/// An invalidation hint arrived.
	fn mark(&mut self, now: Instant) {
		if self.in_flight {
			self.rerun = true;
		} else if self.deadline.is_none() {
			self.deadline = Some(now + self.debounce);
		}
	}

	/// A refresh is owed right now (target change, reconnect).
	fn mark_immediate(&mut self, now: Instant) {
		if self.in_flight {
			self.rerun = true;
		} else {
			self.deadline = Some(now);
		}
	}

	/// When the pending refresh should fire, if one is owed.
	fn deadline(&self) -> Option<Instant> {
		self.deadline
	}

	/// The deadline fired: whether the caller should start a fetch (and the
	/// gate is now in flight).
	fn fire(&mut self) -> bool {
		if self.deadline.is_some() && !self.in_flight {
			self.deadline = None;
			self.in_flight = true;
			true
		} else {
			self.deadline = None;
			false
		}
	}

	/// A fetch finished (either way): whether a rerun was requested while it
	/// ran. A requested rerun re-arms the deadline immediately.
	fn complete(&mut self, now: Instant) -> bool {
		self.in_flight = false;
		if self.rerun {
			self.rerun = false;
			self.deadline = Some(now);
			true
		} else {
			false
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn gate() -> (RefreshGate, Instant) {
		(RefreshGate::new(Duration::from_millis(250)), Instant::now())
	}

	#[test]
	fn first_mark_arms_the_debounce_deadline() {
		let (mut gate, now) = gate();
		assert!(gate.deadline().is_none());
		gate.mark(now);
		assert_eq!(gate.deadline(), Some(now + Duration::from_millis(250)));
	}

	#[test]
	fn later_marks_coalesce_without_sliding_the_deadline() {
		let (mut gate, now) = gate();
		gate.mark(now);
		let armed = gate.deadline();
		gate.mark(now + Duration::from_millis(200));
		assert_eq!(gate.deadline(), armed);
	}

	#[test]
	fn fire_transitions_to_in_flight_exactly_once() {
		let (mut gate, now) = gate();
		gate.mark(now);
		assert!(gate.fire());
		assert!(gate.deadline().is_none());
		assert!(!gate.fire());
	}

	#[test]
	fn marks_during_flight_request_one_rerun() {
		let (mut gate, now) = gate();
		gate.mark_immediate(now);
		assert!(gate.fire());
		gate.mark(now);
		gate.mark(now);
		assert!(gate.deadline().is_none());
		assert!(gate.complete(now));
		assert_eq!(gate.deadline(), Some(now));
		assert!(gate.fire());
		assert!(!gate.complete(now));
		assert!(gate.deadline().is_none());
	}

	#[test]
	fn mark_immediate_skips_the_debounce() {
		let (mut gate, now) = gate();
		gate.mark_immediate(now);
		assert_eq!(gate.deadline(), Some(now));
	}

	#[test]
	fn quiet_completion_owes_nothing() {
		let (mut gate, now) = gate();
		gate.mark(now);
		assert!(gate.fire());
		assert!(!gate.complete(now));
		assert!(gate.deadline().is_none());
		assert!(!gate.fire());
	}

	#[test]
	fn format_bytes_uses_decimal_units() {
		assert_eq!(format_bytes(0), "0 B");
		assert_eq!(format_bytes(999), "999 B");
		assert_eq!(format_bytes(1_000), "1.0 KB");
		assert_eq!(format_bytes(1_500_000), "1.5 MB");
		assert_eq!(format_bytes(494_384_795_648), "494.4 GB");
	}

	#[test]
	fn empty_filters_collapse_in_resource_filter() {
		let filter = resource_filter("file", None, None);
		assert_eq!(filter.resource_type.as_deref(), Some("file"));
		assert!(filter.include_descendants.is_none());
		let scoped = resource_filter(
			"file",
			None,
			Some(SdPath::Physical {
				device_slug: "test".into(),
				path: PathBuf::from("/tmp"),
			}),
		);
		assert_eq!(scoped.include_descendants, Some(false));
	}
}
