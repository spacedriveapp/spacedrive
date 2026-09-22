//! The async data plane between the daemon and the UI.
//!
//! The frame loop never awaits: all daemon traffic happens on a dedicated
//! tokio runtime running on background threads, and the UI reads immutable
//! snapshots out of a watch channel. [`DataHandle`] is the whole surface the
//! views see — snapshot reads, a watch receiver for wakeups, and
//! fire-and-forget commands.
//!
//! Photos follows a window rather than browsing on its own: the plane tracks
//! daemon liveness, this device's slug, and the navigation focus of one group.
//! Focus arrives as a daemon event carrying the whole row, so the common case
//! costs no round trip; a refetch on reconnect covers changes that happened
//! while the socket was down.
//!
//! When focus lands on a folder the plane lists every image and video beneath
//! it, and while the followed window searches, the images and videos its
//! search finds. Listings come from the source stores through `search.media` a
//! page at a time, so the grid draws the first page while the rest lands.
//!
//! The grid is fed from the daemon's thumbnail hot tier. Nothing is baked
//! here: `thumbs.request` names the cells about to be drawn, in draw order,
//! and the daemon bakes what is missing into the cache files this process maps
//! read-only, one per drive. Completions arrive as `thumbnail` events.
//! Identity windows follow the viewport, so a folder of twenty thousand photos
//! costs twenty thousand bakes only if someone scrolls through all of them.
//!
//! Tagging runs through the plane too. The listing carries each cell's record
//! and the tags it carries, requests go out as `tags.apply` and
//! `tags.unapply`, and the file rows the daemon announces afterwards say what
//! each record carries now, for this window's requests and every other
//! client's. Indexing announces rows without reading their tags, so a row
//! naming no tags is believed only for a record this window is changing, and
//! read back otherwise.
//!
//! Daemon liveness is a ping on an interval. The plane never spawns the
//! daemon; offline it simply publishes offline snapshots and keeps retrying.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;

use sd_client::{
	daemon_socket_addr, is_daemon_running, BrokerSubscription, CoreClient, Event, EventFilter,
	SubscriptionBroker,
};
use sd_core::domain::{File, SdPath, Tag};
use sd_core::ops::core::status::output::CoreStatus;
use sd_core::ops::files::query::FileByIdQuery;
use sd_core::ops::navigation::focus::DEFAULT_GROUP;
use sd_core::ops::navigation::get::{NavigationFocusInput, NavigationFocusOutput};
use sd_core::ops::navigation::NavigationFocus;
use sd_core::ops::search::media::{MediaCursor, MediaSearchInput, MediaSearchOutput};
use sd_core::ops::search::{FileSearchInput, SearchFilters, SearchScope};
use sd_core::ops::tags::{
	ApplyTagsInput, SearchTagsInput, SearchTagsOutput, TagTargets, UnapplyTagsInput,
};
use sd_core::ops::thumbs::request::{ThumbRequestInput, ThumbRequestOutput};
use sd_core::service::thumbs::Thumbnail;

use crate::source::{Completion, Entry, Feed, VisibleRange};

/// How often the plane pings the daemon for liveness.
const PING_INTERVAL: Duration = Duration::from_secs(3);

/// How often the viewport is re-read to see whether the identity window
/// should move. Matches the reference renderer's reprioritize cadence: fast
/// enough that a scroll redirects the daemon's queue, slow enough that a
/// flick costs a handful of requests rather than one per frame.
const VIEWPORT_INTERVAL: Duration = Duration::from_millis(100);

/// Cells requested beyond the viewport on each side, so a scroll lands on
/// identities that are already in hand.
const WINDOW_MARGIN: u32 = 256;

/// Files per listing page: the first screenful and well past it, so a folder
/// draws while the rest of it lands.
const PAGE: u32 = 2000;

/// Records one tag request may name. The daemon refuses a longer list.
const MAX_TAG_TARGETS: usize = 1000;

/// Records read back at once, when rows announced for them cannot be believed.
const REREADS_IN_FLIGHT: usize = 8;

/// The resource type file rows are announced under.
const FILE_RESOURCE: &str = "file";

/// Commands the UI sends into the plane. All fire-and-forget.
enum Command {
	SetFollowing(bool),
	/// Put `tag` on `records`, or take it off them.
	Tag {
		tag: Uuid,
		records: Vec<Uuid>,
		apply: bool,
	},
	/// Read the library's tags again.
	RefreshTags,
}

/// A listing ready to render: its first page, and everything the UI needs to
/// build a tile source over the daemon's cache files. Sent once per listing,
/// over its own queue because the channel ends it carries cannot be cloned
/// into a snapshot.
pub struct FolderOpen {
	/// What the toolbar calls the listing: the folder's name, or the search.
	pub title: String,
	/// Growth and identity windows, as pages land and the daemon answers.
	pub feed_rx: std::sync::mpsc::Receiver<Feed>,
	/// Bake completions for cells in this listing.
	pub completions_rx: std::sync::mpsc::Receiver<Completion>,
	/// The viewport the plane reads to decide which identities to ask for.
	pub visible: VisibleRange,
	/// The record each cell of the first page shows, in listing order.
	pub records: Vec<Uuid>,
	/// Each cell's file, in listing order.
	pub paths: Vec<PathBuf>,
	/// The tags each cell's record carries, in listing order.
	pub tags: Vec<Vec<Uuid>>,
	/// What changes about the listing once it is open, queued from the moment
	/// it was listed so none is lost before the UI takes it. Ends with the
	/// listing.
	pub changes: mpsc::UnboundedReceiver<FolderChange>,
}

/// A change to the listing in view. Pages and tag changes share one queue, so
/// a tag change for a record on a later page reaches the UI after the page
/// that holds it.
#[derive(Debug, PartialEq, Eq)]
pub enum FolderChange {
	/// A later page's cells, after every cell before them.
	Appended {
		records: Vec<Uuid>,
		paths: Vec<PathBuf>,
		tags: Vec<Vec<Uuid>>,
	},
	/// What these records carry now.
	Tags(Vec<RecordTags>),
}

/// The color of a tag that has none of its own: the explorer's default blue.
pub const DEFAULT_TAG_COLOR: u32 = 0x3b82f6;

/// A tag as this window shows it: a palette entry, and a dot on every cell
/// whose record carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagInfo {
	pub id: Uuid,
	pub name: String,
	/// `0xRRGGBB`.
	pub color: u32,
}

impl TagInfo {
	fn from_tag(tag: &Tag) -> Self {
		TagInfo {
			id: tag.id,
			name: tag.name.clone(),
			color: tag
				.color
				.as_deref()
				.and_then(parse_hex_color)
				.unwrap_or(DEFAULT_TAG_COLOR),
		}
	}
}

/// The library's tags, and the latest tag request that failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagSnapshot {
	/// Every tag in the library, ordered by path.
	pub tags: Vec<TagInfo>,
	pub notice: Option<Notice>,
}

/// A failure to report, numbered so the window reports each one once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
	pub seq: u64,
	pub message: String,
}

/// Every tag a record carries now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTags {
	pub record: Uuid,
	pub tags: Vec<Uuid>,
}

/// What the plane knows about the window Photos is following.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSnapshot {
	pub online: bool,
	/// Whether focus changes are being applied to the grid.
	pub following: bool,
	/// The focus group this window joined.
	pub group: String,
	/// Where the group is looking, when that is a directory on this device.
	pub path: Option<PathBuf>,
	/// Whether the group is searching, which makes the listing the search's
	/// rather than the folder's.
	pub searching: bool,
	/// How the folder in view is coming along.
	pub folder: FolderState,
}

/// What the plane can say about the folder Photos is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderState {
	/// No folder yet.
	Idle,
	/// The listing is out.
	Loading,
	/// Rendering, with this many cells.
	Ready(u32),
	/// The folder holds no media.
	Empty,
	/// No source reaches the folder, so it has nothing listed to show.
	NoSource,
	/// The publisher named no library, so there is nothing to list through.
	NoLibrary,
	Error(String),
}

impl FocusSnapshot {
	fn new(group: String) -> Self {
		FocusSnapshot {
			online: false,
			following: true,
			group,
			path: None,
			searching: false,
			folder: FolderState::Idle,
		}
	}
}

/// The UI's handle to the data plane. Cloning is cheap; all clones share the
/// same plane.
#[derive(Clone)]
pub struct DataHandle {
	socket_addr: String,
	commands: mpsc::UnboundedSender<Command>,
	focus: watch::Receiver<Arc<FocusSnapshot>>,
	tags: watch::Receiver<Arc<TagSnapshot>>,
	/// Folders the plane has opened and the UI has not picked up. Drained on
	/// the UI thread; the focus watch is what wakes it.
	folders: Arc<std::sync::Mutex<mpsc::UnboundedReceiver<FolderOpen>>>,
}

impl DataHandle {
	/// Current focus snapshot. Non-blocking; safe from the frame loop.
	pub fn focus(&self) -> Arc<FocusSnapshot> {
		self.focus.borrow().clone()
	}

	/// Watch for focus changes. `changed()` is runtime-agnostic, so UI tasks
	/// can await it on gpui's executor.
	pub fn watch_focus(&self) -> watch::Receiver<Arc<FocusSnapshot>> {
		self.focus.clone()
	}

	/// The daemon socket address the plane talks to, for display.
	pub fn socket_addr(&self) -> &str {
		&self.socket_addr
	}

	/// Start or stop applying the followed window's position.
	pub fn set_following(&self, following: bool) {
		let _ = self.commands.send(Command::SetFollowing(following));
	}

	/// The library's tags and the latest tagging failure. Non-blocking; safe
	/// from the frame loop.
	pub fn tags(&self) -> Arc<TagSnapshot> {
		self.tags.borrow().clone()
	}

	/// Watch the tag snapshot, as [`Self::watch_focus`] watches the focus.
	pub fn watch_tags(&self) -> watch::Receiver<Arc<TagSnapshot>> {
		self.tags.clone()
	}

	/// Put `tag` on `records`, or take it off them. Cells change when the
	/// daemon announces the rows, not before.
	pub fn tag(&self, tag: Uuid, records: Vec<Uuid>, apply: bool) {
		let _ = self.commands.send(Command::Tag {
			tag,
			records,
			apply,
		});
	}

	/// Read the library's tags again. Creating a tag announces nothing, so a
	/// tag made in another window arrives this way.
	pub fn refresh_tags(&self) {
		let _ = self.commands.send(Command::RefreshTags);
	}

	/// The next folder the plane has opened, if any. Non-blocking; safe from
	/// the frame loop.
	pub fn take_folder(&self) -> Option<FolderOpen> {
		self.folders
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.try_recv()
			.ok()
	}
}

/// Start the data plane on its own runtime and return the UI handle.
///
/// `instance` selects a named daemon instance (its own port and data
/// directory); `None` targets the default daemon. `group` is the focus group
/// to follow.
pub fn spawn(instance: Option<String>, group: String) -> DataHandle {
	let socket_addr = daemon_socket_addr(instance.as_deref()).to_string();
	let (commands_tx, commands_rx) = mpsc::unbounded_channel();
	let (focus_tx, focus_rx) = watch::channel(Arc::new(FocusSnapshot::new(group.clone())));
	let (tags_tx, tags_rx) = watch::channel(Arc::new(TagSnapshot::default()));
	let (folders_tx, folders_rx) = mpsc::unbounded_channel();

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
			runtime.block_on(run(addr, group, commands_rx, focus_tx, tags_tx, folders_tx));
		})
		.expect("failed to spawn data-plane thread");

	DataHandle {
		socket_addr,
		commands: commands_tx,
		focus: focus_rx,
		tags: tags_rx,
		folders: Arc::new(std::sync::Mutex::new(folders_rx)),
	}
}

/// The focus group this window joins: `SD_NATIVE_FOCUS_GROUP`, or the default
/// group every shell publishes into.
pub fn focus_group_from_env() -> String {
	std::env::var("SD_NATIVE_FOCUS_GROUP")
		.ok()
		.filter(|group| !group.is_empty())
		.unwrap_or_else(|| DEFAULT_GROUP.to_string())
}

/// Everything a completed background task reports into the main loop. Event
/// pumps also land here, so the loop is a single-consumer state machine with
/// no shared mutability.
enum TaskResult {
	Ping(bool),
	SlugFetched(anyhow::Result<String>),
	FocusFetched(anyhow::Result<NavigationFocus>),
	FocusEvent(NavigationFocus),
	/// A page of the listing at `generation` landed, with the request that
	/// asked for it, which the page after it repeats from a cursor.
	Listed {
		generation: u64,
		request: MediaSearchInput,
		result: anyhow::Result<MediaPage>,
	},
	/// Identities for one window landed, in the order they were asked for.
	Identified {
		generation: u64,
		first: u32,
		result: anyhow::Result<ThumbRequestOutput>,
	},
	/// Tiles finished baking.
	Baked(Vec<Thumbnail>),
	/// File rows announced, reduced to the record each names and the tags it
	/// says that record carries.
	FilesChanged(Vec<(Uuid, Vec<Tag>)>),
	/// A record's tags read back, because the row announced for it could not
	/// be believed. `None` when the record is gone.
	Reread {
		generation: u64,
		record: Uuid,
		result: anyhow::Result<Option<Vec<Uuid>>>,
	},
	/// The library's tags landed.
	TagsFetched(anyhow::Result<Vec<Tag>>),
	/// A tag request finished.
	Tagged {
		records: Vec<Uuid>,
		result: anyhow::Result<()>,
	},
}

async fn run(
	socket_addr: String,
	group: String,
	mut commands: mpsc::UnboundedReceiver<Command>,
	focus_tx: watch::Sender<Arc<FocusSnapshot>>,
	tags_tx: watch::Sender<Arc<TagSnapshot>>,
	folders_tx: mpsc::UnboundedSender<FolderOpen>,
) {
	let client = CoreClient::new(socket_addr.clone());
	let broker = SubscriptionBroker::new(socket_addr);
	let (results_tx, mut results) = mpsc::unbounded_channel();

	// Focus is a global resource, not a path-scoped one: the whole point is
	// learning about paths this window is not already watching. The pump lives
	// for the app's lifetime and the broker reconnects it on daemon restarts.
	let focus_events = broker.subscribe(
		resource_event_types(),
		Some(resource_filter("navigation_focus")),
	);
	pump(focus_events, results_tx.clone(), |event| {
		focus_from_event(event).map(TaskResult::FocusEvent)
	});

	// Bake completions for every source; the plane keeps only the ones for
	// cells in the folder it is showing.
	let thumb_events = broker.subscribe(
		resource_event_types(),
		Some(resource_filter(Thumbnail::RESOURCE_TYPE)),
	);
	pump(thumb_events, results_tx.clone(), |event| {
		let baked = thumbnails_from_event(event);
		(!baked.is_empty()).then(|| TaskResult::Baked(baked))
	});

	// File rows, for what they say about tags. This window's tag requests come
	// back this way, and so does every other client's.
	let file_events =
		broker.subscribe(resource_event_types(), Some(resource_filter(FILE_RESOURCE)));
	pump(file_events, results_tx.clone(), |event| {
		let rows = file_rows_from_event(event);
		(!rows.is_empty()).then(|| TaskResult::FilesChanged(rows))
	});

	let mut plane = Plane {
		client,
		results_tx,
		focus_tx,
		tags_tx,
		folders_tx,
		group,
		online: false,
		following: true,
		device_slug: None,
		path: None,
		search: None,
		library_id: None,
		slug_fetch: Fetch::default(),
		focus_fetch: Fetch::default(),
		folder: None,
		folder_state: FolderState::Idle,
		generation: 0,
		library_tags: Vec::new(),
		tags_fetch: Fetch::default(),
		expecting: HashSet::new(),
		rereads: VecDeque::new(),
		rereading: 0,
		notice: None,
	};
	plane.request_slug();

	let mut ping = tokio::time::interval(PING_INTERVAL);
	ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
	let mut viewport = tokio::time::interval(VIEWPORT_INTERVAL);
	viewport.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

	loop {
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
			_ = viewport.tick() => plane.follow_viewport(),
		}
	}
}

/// Forward events on `subscription` into the results channel as whatever
/// `make` builds from them; events it maps to `None` are dropped. Ends when
/// the subscription or the channel closes.
fn pump(
	mut subscription: BrokerSubscription,
	results: mpsc::UnboundedSender<TaskResult>,
	make: impl Fn(&Event) -> Option<TaskResult> + Send + 'static,
) -> JoinHandle<()> {
	tokio::spawn(async move {
		while let Some(event) = subscription.recv().await {
			let Some(result) = make(&event) else {
				continue;
			};
			if results.send(result).is_err() {
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

/// A filter for one resource type, unscoped by path or library.
fn resource_filter(resource_type: &str) -> EventFilter {
	EventFilter {
		library_id: None,
		job_id: None,
		device_id: None,
		resource_type: Some(resource_type.to_string()),
		path_scope: None,
		include_descendants: None,
	}
}

/// The tiles a batched bake-completion event announced. Failures are carried
/// through: a cell whose bake found no producer must stop waiting for one.
fn thumbnails_from_event(event: &Event) -> Vec<Thumbnail> {
	match event {
		Event::ResourceChangedBatch {
			resource_type,
			resources,
			..
		} if resource_type == Thumbnail::RESOURCE_TYPE => {
			serde_json::from_value(resources.clone()).unwrap_or_default()
		}
		Event::ResourceChanged {
			resource_type,
			resource,
			..
		} if resource_type == Thumbnail::RESOURCE_TYPE => serde_json::from_value(resource.clone())
			.map(|one| vec![one])
			.unwrap_or_default(),
		_ => Vec::new(),
	}
}

/// The focus row carried by a resource event, if it is one. The daemon emits
/// focus changes as whole rows, so the follower reads the new position
/// straight off the event instead of asking for it.
fn focus_from_event(event: &Event) -> Option<NavigationFocus> {
	match event {
		Event::ResourceChanged {
			resource_type,
			resource,
			..
		} if resource_type == NavigationFocus::RESOURCE_TYPE => {
			serde_json::from_value(resource.clone()).ok()
		}
		_ => None,
	}
}

/// The record and tags of every file row an event carries. Only those two
/// fields are read: indexing announces rows by the thousand, and tagging
/// needs nothing else from them.
fn file_rows_from_event(event: &Event) -> Vec<(Uuid, Vec<Tag>)> {
	match event {
		Event::ResourceChanged {
			resource_type,
			resource,
			..
		} if resource_type == FILE_RESOURCE => file_row(resource).into_iter().collect(),
		Event::ResourceChangedBatch {
			resource_type,
			resources,
			..
		} if resource_type == FILE_RESOURCE => resources
			.as_array()
			.map(|rows| rows.iter().filter_map(file_row).collect())
			.unwrap_or_default(),
		_ => Vec::new(),
	}
}

fn file_row(row: &serde_json::Value) -> Option<(Uuid, Vec<Tag>)> {
	let record = serde_json::from_value(row.get("id")?.clone()).ok()?;
	let tags = serde_json::from_value(row.get("tags")?.clone()).ok()?;
	Some((record, tags))
}

/// What an announced row means for the tags a record in view carries.
#[derive(Debug, PartialEq, Eq)]
enum Reading {
	/// The record carries these tags now.
	Set(Vec<Uuid>),
	/// The row cannot say; the record has to be read back.
	Reread,
	/// Nothing changes.
	Keep,
}

/// Read a row announced for a record that carries `current`. `expected` is
/// whether a tag request from this window is changing the record.
///
/// Rows from a tag change are read with their tags, and a row naming a tag
/// can only be one of those. Indexing announces rows without reading tags,
/// so a row naming none is believed only when this window's own request
/// explains it. Otherwise it says nothing about a record that has none, and
/// for one that has some, only the daemon can settle it.
fn read_row(current: &[Uuid], announced: &[Tag], expected: bool) -> Reading {
	if !announced.is_empty() || expected {
		Reading::Set(announced.iter().map(|tag| tag.id).collect())
	} else if current.is_empty() {
		Reading::Keep
	} else {
		Reading::Reread
	}
}

/// One listing being rendered, and the wiring feeding it.
struct Folder {
	/// Every media path listed so far, in listing order. Index is grid index.
	paths: Vec<PathBuf>,
	/// Growth and identity windows out to the grid.
	feed_tx: std::sync::mpsc::Sender<Feed>,
	/// Completions out to the grid.
	completions_tx: std::sync::mpsc::Sender<Completion>,
	/// The viewport, written by the grid and read here.
	visible: VisibleRange,
	/// Grid index for every identity the daemon has answered with, so a bake
	/// completion for a uuid finds the cell showing it.
	index_by_uuid: HashMap<Uuid, u32>,
	/// The window already asked for, so a still viewport costs nothing.
	requested: Option<(u32, u32)>,
	/// One identity request in flight at a time.
	identifying: bool,
	/// The tags of every record listed, as the UI was last told.
	tags: HashMap<Uuid, Vec<Uuid>>,
	/// Later pages and tag changes out to the UI.
	changes_tx: mpsc::UnboundedSender<FolderChange>,
}

impl Folder {
	/// Take `tags` as what `record` carries. Returns the change the UI needs,
	/// or `None` when it already has it or the record is not in this folder.
	fn set_tags(&mut self, record: Uuid, tags: Vec<Uuid>) -> Option<RecordTags> {
		let current = self.tags.get_mut(&record)?;
		if *current == tags {
			return None;
		}
		current.clone_from(&tags);
		Some(RecordTags { record, tags })
	}
}

/// The single-threaded state machine behind the select loop.
struct Plane {
	client: CoreClient,
	results_tx: mpsc::UnboundedSender<TaskResult>,
	focus_tx: watch::Sender<Arc<FocusSnapshot>>,
	tags_tx: watch::Sender<Arc<TagSnapshot>>,
	folders_tx: mpsc::UnboundedSender<FolderOpen>,

	/// The focus group this window follows.
	group: String,
	online: bool,
	following: bool,
	/// This daemon's device slug, learned from `core.status`. A focus row for
	/// another device names a path that does not exist here, so the slug is
	/// what makes a published position usable.
	device_slug: Option<String>,
	path: Option<PathBuf>,
	/// The search the followed window is running, which is listed in place
	/// of its folder.
	search: Option<FileSearchInput>,
	/// The library the followed window is in; listings route through it.
	library_id: Option<Uuid>,

	slug_fetch: Fetch,
	focus_fetch: Fetch,

	folder: Option<Folder>,
	folder_state: FolderState,
	/// Bumped on every folder change; results for an older folder are dropped.
	generation: u64,

	/// Every tag in the followed window's library, ordered by path.
	library_tags: Vec<TagInfo>,
	tags_fetch: Fetch,
	/// Records a tag request from this window is changing. The next row
	/// announced for each is believed even when it names no tags.
	expecting: HashSet<Uuid>,
	/// Records to read back, and how many reads are out.
	rereads: VecDeque<Uuid>,
	rereading: usize,
	notice: Option<Notice>,
}

impl Plane {
	fn handle_command(&mut self, command: Command) {
		match command {
			Command::SetFollowing(following) => {
				if self.following == following {
					return;
				}
				self.following = following;
				// Resuming adopts wherever the group has moved to meanwhile.
				if following {
					self.request_focus();
				}
				self.publish();
			}
			Command::Tag {
				tag,
				records,
				apply,
			} => self.tag(tag, records, apply),
			Command::RefreshTags => self.request_tags(),
		}
	}

	fn handle_result(&mut self, result: TaskResult) {
		match result {
			TaskResult::Ping(alive) => self.handle_ping(alive),
			TaskResult::SlugFetched(result) => self.handle_slug_fetched(result),
			TaskResult::FocusFetched(result) => {
				match result {
					Ok(focus) => self.apply_focus(focus),
					Err(error) => eprintln!("spacedrive-native: focus refresh failed: {error:#}"),
				}
				if self.focus_fetch.complete() {
					self.request_focus();
				}
			}
			TaskResult::FocusEvent(focus) => self.apply_focus(focus),
			TaskResult::Listed {
				generation,
				request,
				result,
			} => self.handle_listed(generation, request, result),
			TaskResult::Identified {
				generation,
				first,
				result,
			} => self.handle_identified(generation, first, result),
			TaskResult::Baked(tiles) => self.handle_baked(tiles),
			TaskResult::FilesChanged(rows) => self.handle_files_changed(rows),
			TaskResult::Reread {
				generation,
				record,
				result,
			} => self.handle_reread(generation, record, result),
			TaskResult::TagsFetched(result) => self.handle_tags_fetched(result),
			TaskResult::Tagged { records, result } => self.handle_tagged(records, result),
		}
	}

	/// A page landed. The first opens the listing in the UI and asks for the
	/// identities of the first screenful; a later one grows it. Either asks for
	/// the page after it, so a listing fills in while its first cells draw.
	fn handle_listed(
		&mut self,
		generation: u64,
		request: MediaSearchInput,
		result: anyhow::Result<MediaPage>,
	) {
		if generation != self.generation {
			return;
		}
		let page = match result {
			Ok(page) => page,
			Err(error) => {
				self.folder_state = FolderState::Error(format!("{error:#}"));
				self.publish();
				return;
			}
		};
		let next = page.next.clone();
		if self.folder.is_some() {
			self.grow_listing(page.media);
		} else if !page.covered {
			self.folder_state = FolderState::NoSource;
			self.publish();
			return;
		} else if page.media.is_empty() {
			self.folder_state = FolderState::Empty;
			self.publish();
			return;
		} else {
			self.open_listing(page.media);
		}
		if let (Some(library_id), Some(after)) = (self.library_id, next) {
			self.request_page(
				library_id,
				MediaSearchInput {
					after: Some(after),
					..request
				},
			);
		}
	}

	/// Hand a listing's first page to the UI and ask for the identities of the
	/// first screenful.
	fn open_listing(&mut self, media: Vec<Media>) {
		let (feed_tx, feed_rx) = std::sync::mpsc::channel();
		let (completions_tx, completions_rx) = std::sync::mpsc::channel();
		let (changes_tx, changes) = mpsc::unbounded_channel();
		let visible = VisibleRange::new(0, WINDOW_MARGIN);
		let (records, paths, tags) = cells_of(media);
		if tags
			.iter()
			.any(|tags| names_unknown(&self.library_tags, tags))
		{
			self.request_tags();
		}
		let len = paths.len() as u32;

		self.folder = Some(Folder {
			paths: paths.clone(),
			feed_tx,
			completions_tx,
			visible: visible.clone(),
			index_by_uuid: HashMap::new(),
			requested: None,
			identifying: false,
			tags: records.iter().copied().zip(tags.iter().cloned()).collect(),
			changes_tx,
		});
		let _ = self.folders_tx.send(FolderOpen {
			title: self.title(),
			feed_rx,
			completions_rx,
			visible,
			records,
			paths,
			tags,
			changes,
		});
		self.folder_state = FolderState::Ready(len);
		self.publish();
		self.request_window(0, WINDOW_MARGIN.min(len));
	}

	/// Take a later page onto the end of the listing in view.
	fn grow_listing(&mut self, media: Vec<Media>) {
		if media.is_empty() {
			return;
		}
		let (records, paths, tags) = cells_of(media);
		if tags
			.iter()
			.any(|tags| names_unknown(&self.library_tags, tags))
		{
			self.request_tags();
		}
		let Some(folder) = self.folder.as_mut() else {
			return;
		};
		folder.paths.extend(paths.iter().cloned());
		folder
			.tags
			.extend(records.iter().copied().zip(tags.iter().cloned()));
		let len = folder.paths.len() as u32;
		let _ = folder.feed_tx.send(Feed::Grew(len));
		let _ = folder.changes_tx.send(FolderChange::Appended {
			records,
			paths,
			tags,
		});
		self.folder_state = FolderState::Ready(len);
		self.publish();
	}

	/// Identities for one window landed, each naming the cache its tile lives
	/// in, with the files behind those caches.
	fn handle_identified(
		&mut self,
		generation: u64,
		first: u32,
		result: anyhow::Result<ThumbRequestOutput>,
	) {
		if generation != self.generation {
			return;
		}
		let Some(folder) = self.folder.as_mut() else {
			return;
		};
		folder.identifying = false;
		let output = match result {
			Ok(output) => output,
			Err(error) => {
				self.folder_state = FolderState::Error(format!("{error:#}"));
				self.publish();
				return;
			}
		};

		let entries: Vec<(u32, Entry)> = output
			.tiles
			.iter()
			.enumerate()
			.filter_map(|(offset, tile)| {
				let tile = tile.as_ref()?;
				Some((
					first + offset as u32,
					Entry {
						cache: tile.source_id,
						uuid: tile.uuid,
						version: tile.version,
					},
				))
			})
			.collect();
		for (index, entry) in &entries {
			folder.index_by_uuid.insert(entry.uuid, *index);
		}
		let caches = output
			.sources
			.into_iter()
			.map(|source| (source.id, source.cache_path))
			.collect();
		let _ = folder.feed_tx.send(Feed::Window { caches, entries });
	}

	/// Bake completions: wake the cells showing them.
	fn handle_baked(&mut self, tiles: Vec<Thumbnail>) {
		let Some(folder) = self.folder.as_ref() else {
			return;
		};
		for tile in tiles {
			if let Some(index) = folder.index_by_uuid.get(&tile.id) {
				let _ = folder.completions_tx.send(Completion {
					index: *index,
					ok: tile.ok,
				});
			}
		}
	}

	/// Ask the daemon for identities covering the viewport, when it has moved
	/// past the window already in hand.
	fn follow_viewport(&mut self) {
		let Some(folder) = self.folder.as_ref() else {
			return;
		};
		if folder.identifying {
			return;
		}
		let len = folder.paths.len() as u32;
		let (visible_first, visible_last) = folder.visible.get();
		let first = visible_first.saturating_sub(WINDOW_MARGIN);
		let last = visible_last.saturating_add(WINDOW_MARGIN).min(len);
		if first >= last || folder.requested == Some((first, last)) {
			return;
		}
		self.request_window(first, last);
	}

	/// Send one identity request for `[first, last)`.
	fn request_window(&mut self, first: u32, last: u32) {
		let Some(folder) = self.folder.as_mut() else {
			return;
		};
		let device_slug = self.device_slug.clone().unwrap_or_default();
		let paths: Vec<SdPath> = folder.paths[first as usize..last as usize]
			.iter()
			.map(|path| SdPath::Physical {
				device_slug: device_slug.clone(),
				path: path.clone(),
			})
			.collect();
		if paths.is_empty() {
			return;
		}
		folder.requested = Some((first, last));
		folder.identifying = true;

		let client = self.client.clone();
		let results = self.results_tx.clone();
		let generation = self.generation;
		let library_id = self.library_id;
		tokio::spawn(async move {
			let result = client
				.action(&ThumbRequestInput { paths }, library_id)
				.await
				.and_then(|value| {
					serde_json::from_value::<ThumbRequestOutput>(value).map_err(Into::into)
				});
			let _ = results.send(TaskResult::Identified {
				generation,
				first,
				result,
			});
		});
	}

	fn handle_ping(&mut self, alive: bool) {
		if alive == self.online {
			return;
		}
		self.online = alive;
		if alive {
			// A reconnect may have missed changes entirely, so the position is
			// re-read rather than waited for.
			self.request_slug();
			self.request_focus();
			self.request_tags();
		}
		self.publish();
	}

	fn handle_slug_fetched(&mut self, result: anyhow::Result<String>) {
		match result {
			Ok(slug) => {
				let learned = self.device_slug.as_deref() != Some(&slug);
				self.online = true;
				self.device_slug = Some(slug);
				if learned {
					// Focus rows could not be resolved to a local path before.
					self.request_focus();
					if self.folder.is_none() && (self.path.is_some() || self.search.is_some()) {
						self.open_folder();
					}
				}
				self.publish();
			}
			Err(error) => {
				// The ping loop owns the offline transition; a failed fetch
				// alone (e.g. mid-shutdown) just logs.
				eprintln!("spacedrive-native: device status failed: {error:#}");
			}
		}
		if self.slug_fetch.complete() {
			self.request_slug();
		}
	}

	/// Take a published position if it belongs to this window's group: a
	/// directory on this device, a search, or both.
	fn apply_focus(&mut self, focus: NavigationFocus) {
		if focus.group != self.group {
			return;
		}
		let library_id = focus.library_id;
		let path = focus.path.and_then(|path| self.local_path(path));
		let search = focus.search;
		if self.path == path && self.search == search && self.library_id == library_id {
			return;
		}
		let library_changed = self.library_id != library_id;
		self.path = path;
		self.search = search;
		self.library_id = library_id;
		if library_changed {
			// Tags belong to a library, so another library's are not these.
			self.library_tags.clear();
			self.publish_tags();
			self.request_tags();
		}
		self.open_folder();
	}

	/// List what the followed window is looking at and start rendering it. A
	/// change abandons the previous listing: its results carry an older
	/// generation and are dropped, and dropping its `Folder` closes the
	/// channels feeding the grid, which is how the old tile source learns it
	/// is finished.
	fn open_folder(&mut self) {
		self.generation += 1;
		self.folder = None;
		self.rereads.clear();

		if self.path.is_none() && self.search.is_none() {
			self.folder_state = FolderState::Idle;
			self.publish();
			return;
		}
		let Some(library_id) = self.library_id else {
			self.folder_state = FolderState::NoLibrary;
			self.publish();
			return;
		};
		// The listing's cells cannot be addressed yet; the slug fetch marks
		// this listing for another attempt when it lands.
		let Some(request) = self
			.device_slug
			.as_deref()
			.and_then(|slug| self.first_page(slug))
		else {
			self.folder_state = FolderState::Loading;
			self.publish();
			return;
		};

		self.folder_state = FolderState::Loading;
		self.publish();
		self.request_page(library_id, request);
	}

	/// The first page of what the followed window shows: the media its search
	/// finds while it searches, and otherwise the media beneath its folder.
	fn first_page(&self, device_slug: &str) -> Option<MediaSearchInput> {
		let (query, scope, filters) = match (&self.search, &self.path) {
			(Some(search), _) => (
				search.query.clone(),
				search.scope.clone(),
				search.filters.clone(),
			),
			(None, Some(path)) => (
				String::new(),
				SearchScope::Path {
					path: SdPath::Physical {
						device_slug: device_slug.to_string(),
						path: path.clone(),
					},
				},
				SearchFilters::default(),
			),
			(None, None) => return None,
		};
		Some(MediaSearchInput {
			query,
			scope,
			filters,
			after: None,
			limit: PAGE,
		})
	}

	/// Ask for one page of the listing in view.
	fn request_page(&self, library_id: Uuid, request: MediaSearchInput) {
		let client = self.client.clone();
		let results = self.results_tx.clone();
		let generation = self.generation;
		tokio::spawn(async move {
			let result = list_page(&client, library_id, &request).await;
			let _ = results.send(TaskResult::Listed {
				generation,
				request,
				result,
			});
		});
	}

	/// What the toolbar calls the listing in view.
	fn title(&self) -> String {
		match (&self.search, &self.path) {
			(Some(search), _) if !search.query.is_empty() => format!("“{}”", search.query),
			(Some(_), _) => "Search".to_string(),
			(None, Some(path)) => path
				.file_name()
				.map(|name| name.to_string_lossy().into_owned())
				.unwrap_or_else(|| path.display().to_string()),
			(None, None) => String::new(),
		}
	}

	/// The local directory an `SdPath` names, or `None` when it belongs to
	/// another device or has no filesystem path at all.
	fn local_path(&self, path: SdPath) -> Option<PathBuf> {
		match path {
			SdPath::Physical { device_slug, path } => {
				match self.device_slug.as_deref() {
					// Before the slug is known, a physical path is taken at
					// face value; the refetch after it lands corrects this.
					None => Some(path),
					Some(slug) if slug == device_slug => Some(path),
					Some(_) => None,
				}
			}
			_ => None,
		}
	}

	fn spawn_ping(&self) {
		let client = self.client.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let _ = results.send(TaskResult::Ping(is_daemon_running(&client).await));
		});
	}

	fn request_slug(&mut self) {
		if !self.slug_fetch.request() {
			return;
		}
		let client = self.client.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let result = client
				.query::<_, CoreStatus>(&(), None)
				.await
				.map(|status| status.device_info.slug);
			let _ = results.send(TaskResult::SlugFetched(result));
		});
	}

	fn request_focus(&mut self) {
		if !self.focus_fetch.request() {
			return;
		}
		let client = self.client.clone();
		let results = self.results_tx.clone();
		let group = self.group.clone();
		tokio::spawn(async move {
			let input = NavigationFocusInput { group: Some(group) };
			let result = client
				.query::<_, NavigationFocusOutput>(&input, None)
				.await
				.map(|output| output.focus);
			let _ = results.send(TaskResult::FocusFetched(result));
		});
	}

	fn publish(&self) {
		let _ = self.focus_tx.send(Arc::new(FocusSnapshot {
			online: self.online,
			following: self.following,
			group: self.group.clone(),
			path: self.path.clone(),
			searching: self.search.is_some(),
			folder: self.folder_state.clone(),
		}));
	}

	/// Rows the daemon announced: pass on what they say about the tags of
	/// records in view, where it can be believed.
	fn handle_files_changed(&mut self, rows: Vec<(Uuid, Vec<Tag>)>) {
		let Some(folder) = self.folder.as_mut() else {
			return;
		};
		let mut changes = Vec::new();
		let mut unknown = false;
		for (record, announced) in rows {
			let Some(current) = folder.tags.get(&record) else {
				continue;
			};
			let expected = self.expecting.remove(&record);
			match read_row(current, &announced, expected) {
				Reading::Set(tags) => {
					unknown |= names_unknown(&self.library_tags, &tags);
					changes.extend(folder.set_tags(record, tags));
				}
				Reading::Reread => {
					if !self.rereads.contains(&record) {
						self.rereads.push_back(record);
					}
				}
				Reading::Keep => {}
			}
		}
		if !changes.is_empty() {
			let _ = folder.changes_tx.send(FolderChange::Tags(changes));
		}
		if unknown {
			self.request_tags();
		}
		self.pump_rereads();
	}

	/// A record's tags read back: what it carries now, from the daemon itself.
	fn handle_reread(
		&mut self,
		generation: u64,
		record: Uuid,
		result: anyhow::Result<Option<Vec<Uuid>>>,
	) {
		self.rereading -= 1;
		if generation == self.generation {
			match result {
				Ok(Some(tags)) => {
					if names_unknown(&self.library_tags, &tags) {
						self.request_tags();
					}
					if let Some(folder) = self.folder.as_mut() {
						if let Some(change) = folder.set_tags(record, tags) {
							let _ = folder.changes_tx.send(FolderChange::Tags(vec![change]));
						}
					}
				}
				// The record is gone. Its cell keeps what it showed until the
				// folder is listed again.
				Ok(None) => {}
				Err(error) => eprintln!("spacedrive-native: tag read-back failed: {error:#}"),
			}
		}
		self.pump_rereads();
	}

	/// Start read-backs up to the limit.
	fn pump_rereads(&mut self) {
		let Some(library_id) = self.library_id else {
			return;
		};
		while self.rereading < REREADS_IN_FLIGHT {
			let Some(record) = self.rereads.pop_front() else {
				break;
			};
			self.rereading += 1;
			let client = self.client.clone();
			let results = self.results_tx.clone();
			let generation = self.generation;
			tokio::spawn(async move {
				let result = client
					.query::<_, Option<File>>(&FileByIdQuery::new(record), Some(library_id))
					.await
					.map(|file| file.map(|file| file.tags.iter().map(|tag| tag.id).collect()));
				let _ = results.send(TaskResult::Reread {
					generation,
					record,
					result,
				});
			});
		}
	}

	/// Put `tag` on `records` or take it off. Nothing changes here until the
	/// daemon announces the rows.
	fn tag(&mut self, tag: Uuid, records: Vec<Uuid>, apply: bool) {
		let Some(library_id) = self.library_id else {
			return;
		};
		if records.is_empty() {
			return;
		}
		self.expecting.extend(records.iter().copied());
		let client = self.client.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let result = send_tag(&client, library_id, tag, &records, apply).await;
			let _ = results.send(TaskResult::Tagged { records, result });
		});
	}

	fn handle_tagged(&mut self, records: Vec<Uuid>, result: anyhow::Result<()>) {
		// A row that arrives after this is read like any other: believed when
		// it names tags, read back when it names none.
		for record in &records {
			self.expecting.remove(record);
		}
		if let Err(error) = result {
			self.report(tag_failure_message(&format!("{error:#}")));
		}
	}

	fn report(&mut self, message: String) {
		let seq = self.notice.as_ref().map_or(1, |notice| notice.seq + 1);
		self.notice = Some(Notice { seq, message });
		self.publish_tags();
	}

	fn request_tags(&mut self) {
		let Some(library_id) = self.library_id else {
			return;
		};
		if !self.tags_fetch.request() {
			return;
		}
		let client = self.client.clone();
		let results = self.results_tx.clone();
		tokio::spawn(async move {
			let input = SearchTagsInput {
				query: String::new(),
				limit: None,
			};
			let result = client
				.query::<_, SearchTagsOutput>(&input, Some(library_id))
				.await
				.map(|output| output.tags);
			let _ = results.send(TaskResult::TagsFetched(result));
		});
	}

	fn handle_tags_fetched(&mut self, result: anyhow::Result<Vec<Tag>>) {
		match result {
			Ok(tags) => {
				let tags: Vec<TagInfo> = tags.iter().map(TagInfo::from_tag).collect();
				if tags != self.library_tags {
					self.library_tags = tags;
					self.publish_tags();
				}
			}
			Err(error) => eprintln!("spacedrive-native: tag listing failed: {error:#}"),
		}
		if self.tags_fetch.complete() {
			self.request_tags();
		}
	}

	fn publish_tags(&self) {
		let _ = self.tags_tx.send(Arc::new(TagSnapshot {
			tags: self.library_tags.clone(),
			notice: self.notice.clone(),
		}));
	}
}

/// Whether `tags` names any tag missing from `library`, which means the
/// library's tags were read before that one was made.
fn names_unknown(library: &[TagInfo], tags: &[Uuid]) -> bool {
	tags.iter()
		.any(|id| !library.iter().any(|tag| tag.id == *id))
}

/// Put `tag` on `records` or take it off, a batch the daemon accepts at a
/// time.
async fn send_tag(
	client: &CoreClient,
	library_id: Uuid,
	tag: Uuid,
	records: &[Uuid],
	apply: bool,
) -> anyhow::Result<()> {
	for batch in records.chunks(MAX_TAG_TARGETS) {
		let targets = TagTargets::File(batch.to_vec());
		let tag_ids = vec![tag];
		if apply {
			client
				.action(&ApplyTagsInput { targets, tag_ids }, Some(library_id))
				.await?;
		} else {
			client
				.action(&UnapplyTagsInput { targets, tag_ids }, Some(library_id))
				.await?;
		}
	}
	Ok(())
}

/// What the window says when a tag request fails. A file outside every
/// tracked source is the one refusal a person can act on, so it gets words of
/// its own.
fn tag_failure_message(error: &str) -> String {
	if error.contains("no tracked source") {
		"Tags live in a source. Add this folder to your library first.".to_string()
	} else {
		format!("Failed to toggle tag: {error}")
	}
}

/// `#rrggbb` as `0xRRGGBB`, the form tag colors are stored in.
fn parse_hex_color(color: &str) -> Option<u32> {
	let hex = color.strip_prefix('#').unwrap_or(color);
	if hex.len() != 6 {
		return None;
	}
	u32::from_str_radix(hex, 16).ok()
}

/// One cell of a listing: the file, the record it is, and the tags that
/// record carries.
struct Media {
	path: PathBuf,
	record: Uuid,
	tags: Vec<Uuid>,
}

/// One page of a listing, as the grid draws it.
struct MediaPage {
	media: Vec<Media>,
	/// Where the next page starts; `None` after the last.
	next: Option<MediaCursor>,
	/// Whether any source reaches what was listed.
	covered: bool,
}

/// Fetch one page of a listing. The daemon keeps only images and videos, in
/// the order the grid draws them.
async fn list_page(
	client: &CoreClient,
	library_id: Uuid,
	request: &MediaSearchInput,
) -> anyhow::Result<MediaPage> {
	let output: MediaSearchOutput = client.query(request, Some(library_id)).await?;
	Ok(MediaPage {
		media: output
			.files
			.into_iter()
			.filter_map(|file| match file.sd_path {
				SdPath::Physical { path, .. } => Some(Media {
					path,
					record: file.id,
					tags: file.tags.iter().map(|tag| tag.id).collect(),
				}),
				_ => None,
			})
			.collect(),
		next: output.next,
		covered: output.covered,
	})
}

/// A page's cells as the grid keeps them: records, files and tags, each in
/// listing order.
fn cells_of(media: Vec<Media>) -> (Vec<Uuid>, Vec<PathBuf>, Vec<Vec<Uuid>>) {
	let mut records = Vec::with_capacity(media.len());
	let mut paths = Vec::with_capacity(media.len());
	let mut tags = Vec::with_capacity(media.len());
	for item in media {
		records.push(item.record);
		paths.push(item.path);
		tags.push(item.tags);
	}
	(records, paths, tags)
}

/// One outstanding daemon fetch at a time, with at most one queued rerun.
///
/// Focus changes arrive as events, so a fetch is only ever owed on a
/// reconnect or a resume — rare enough that a request either starts now or
/// waits for the one in flight to finish.
#[derive(Default)]
struct Fetch {
	in_flight: bool,
	rerun: bool,
}

impl Fetch {
	/// A fetch is owed: whether the caller should start one now.
	fn request(&mut self) -> bool {
		if self.in_flight {
			self.rerun = true;
			false
		} else {
			self.in_flight = true;
			true
		}
	}

	/// A fetch finished (either way): whether a rerun was requested while it
	/// ran.
	fn complete(&mut self) -> bool {
		self.in_flight = false;
		std::mem::take(&mut self.rerun)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	fn plane_at(slug: Option<&str>) -> Plane {
		let (results_tx, _results_rx) = mpsc::unbounded_channel();
		let (focus_tx, _focus_rx) = watch::channel(Arc::new(FocusSnapshot::new("default".into())));
		let (tags_tx, _tags_rx) = watch::channel(Arc::new(TagSnapshot::default()));
		let (folders_tx, _folders_rx) = mpsc::unbounded_channel();
		Plane {
			client: CoreClient::new("127.0.0.1:0".to_string()),
			results_tx,
			focus_tx,
			tags_tx,
			folders_tx,
			group: "default".to_string(),
			online: true,
			following: true,
			device_slug: slug.map(String::from),
			path: None,
			search: None,
			library_id: None,
			slug_fetch: Fetch::default(),
			focus_fetch: Fetch::default(),
			folder: None,
			folder_state: FolderState::Idle,
			generation: 0,
			library_tags: Vec::new(),
			tags_fetch: Fetch::default(),
			expecting: HashSet::new(),
			rereads: VecDeque::new(),
			rereading: 0,
			notice: None,
		}
	}

	/// A plane showing a listing whose records carry `tags`, and the receiving
	/// ends of the changes and the feed it sends the UI. No library is set, so
	/// nothing here reaches for a daemon.
	fn plane_showing(
		tags: &[(Uuid, Vec<Uuid>)],
	) -> (
		Plane,
		mpsc::UnboundedReceiver<FolderChange>,
		std::sync::mpsc::Receiver<Feed>,
	) {
		let mut plane = plane_at(Some("laptop"));
		let (feed_tx, feed) = std::sync::mpsc::channel();
		let (completions_tx, _) = std::sync::mpsc::channel();
		let (changes_tx, changes) = mpsc::unbounded_channel();
		plane.folder = Some(Folder {
			paths: vec![PathBuf::from("/photos/a.jpg")],
			feed_tx,
			completions_tx,
			visible: VisibleRange::new(0, 0),
			index_by_uuid: HashMap::new(),
			requested: None,
			identifying: false,
			tags: tags.iter().cloned().collect(),
			changes_tx,
		});
		(plane, changes, feed)
	}

	fn tag(id: Uuid, name: &str) -> Tag {
		Tag {
			id,
			path: format!("Places/{name}"),
			name: name.to_string(),
			color: Some("#3b82f6".to_string()),
			icon: None,
		}
	}

	fn file_json(record: Uuid, tags: &[Tag]) -> serde_json::Value {
		serde_json::json!({ "id": record, "name": "IMG_0001", "tags": tags, "size": 42 })
	}

	fn focus(group: &str, device_slug: &str, path: &str) -> NavigationFocus {
		NavigationFocus {
			id: NavigationFocus::id_for_group(group),
			group: group.to_string(),
			path: Some(SdPath::Physical {
				device_slug: device_slug.to_string(),
				path: PathBuf::from(path),
			}),
			search: None,
			library_id: None,
			origin: Some("test".into()),
			updated_at: Utc::now(),
		}
	}

	#[test]
	fn a_request_while_one_is_in_flight_queues_exactly_one_rerun() {
		let mut fetch = Fetch::default();
		assert!(fetch.request());
		assert!(!fetch.request());
		assert!(!fetch.request());
		assert!(fetch.complete());
		assert!(fetch.request());
		assert!(!fetch.complete());
	}

	#[test]
	fn a_quiet_fetch_owes_no_rerun() {
		let mut fetch = Fetch::default();
		assert!(fetch.request());
		assert!(!fetch.complete());
		assert!(fetch.request());
	}

	#[test]
	fn a_focus_event_carries_the_whole_row() {
		let published = focus("default", "laptop", "/photos");
		let event = Event::ResourceChanged {
			resource_type: NavigationFocus::RESOURCE_TYPE.to_string(),
			resource: serde_json::to_value(&published).expect("focus serializes"),
			metadata: None,
		};
		assert_eq!(focus_from_event(&event), Some(published));

		let other = Event::ResourceChanged {
			resource_type: "file".to_string(),
			resource: serde_json::Value::Null,
			metadata: None,
		};
		assert_eq!(focus_from_event(&other), None);
	}

	#[test]
	fn another_group_is_ignored() {
		let mut plane = plane_at(Some("laptop"));
		plane.apply_focus(focus("second-window", "laptop", "/photos"));
		assert_eq!(plane.path, None);
		plane.apply_focus(focus("default", "laptop", "/photos"));
		assert_eq!(plane.path, Some(PathBuf::from("/photos")));
	}

	#[test]
	fn another_device_is_ignored() {
		let mut plane = plane_at(Some("laptop"));
		plane.apply_focus(focus("default", "desktop", "/photos"));
		assert_eq!(plane.path, None);
	}

	#[test]
	fn a_physical_path_is_taken_before_the_slug_is_known() {
		let mut plane = plane_at(None);
		plane.apply_focus(focus("default", "desktop", "/photos"));
		assert_eq!(plane.path, Some(PathBuf::from("/photos")));
	}

	#[test]
	fn file_rows_are_read_from_single_and_batched_events() {
		let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
		let beach = tag(Uuid::new_v4(), "Beach");

		let single = Event::ResourceChanged {
			resource_type: FILE_RESOURCE.to_string(),
			resource: file_json(first, std::slice::from_ref(&beach)),
			metadata: None,
		};
		assert_eq!(
			file_rows_from_event(&single),
			vec![(first, vec![beach.clone()])]
		);

		let batch = Event::ResourceChangedBatch {
			resource_type: FILE_RESOURCE.to_string(),
			resources: serde_json::json!([file_json(first, &[]), file_json(second, &[beach])]),
			metadata: None,
		};
		let rows = file_rows_from_event(&batch);
		assert_eq!(rows.len(), 2);
		assert_eq!(rows[0], (first, Vec::new()));
		assert_eq!(rows[1].0, second);

		let other = Event::ResourceChanged {
			resource_type: "tag".to_string(),
			resource: file_json(first, &[]),
			metadata: None,
		};
		assert!(file_rows_from_event(&other).is_empty());
	}

	#[test]
	fn a_row_naming_tags_is_believed() {
		let beach = tag(Uuid::new_v4(), "Beach");
		assert_eq!(
			read_row(&[], std::slice::from_ref(&beach), false),
			Reading::Set(vec![beach.id])
		);
	}

	#[test]
	fn a_row_naming_no_tags_is_believed_only_for_this_windows_own_change() {
		let tagged = [Uuid::new_v4()];
		assert_eq!(read_row(&tagged, &[], true), Reading::Set(Vec::new()));
		assert_eq!(read_row(&tagged, &[], false), Reading::Reread);
		assert_eq!(read_row(&[], &[], false), Reading::Keep);
	}

	#[test]
	fn announced_tags_reach_the_ui_once() {
		let record = Uuid::new_v4();
		let beach = tag(Uuid::new_v4(), "Beach");
		let (mut plane, mut changes, _feed) = plane_showing(&[(record, Vec::new())]);

		plane.handle_files_changed(vec![(record, vec![beach.clone()])]);
		assert_eq!(
			changes.try_recv().expect("a change is sent"),
			FolderChange::Tags(vec![RecordTags {
				record,
				tags: vec![beach.id],
			}])
		);

		// The same row again changes nothing the UI does not already have.
		plane.handle_files_changed(vec![(record, vec![beach])]);
		assert!(changes.try_recv().is_err());
	}

	#[test]
	fn an_indexing_row_does_not_untag_a_record() {
		let record = Uuid::new_v4();
		let (mut plane, mut changes, _feed) = plane_showing(&[(record, vec![Uuid::new_v4()])]);

		plane.handle_files_changed(vec![(record, Vec::new())]);
		assert!(changes.try_recv().is_err());
		assert_eq!(plane.rereads, VecDeque::from([record]));

		// A second row for the same record does not queue a second read.
		plane.handle_files_changed(vec![(record, Vec::new())]);
		assert_eq!(plane.rereads.len(), 1);
	}

	#[test]
	fn this_windows_own_untagging_is_believed() {
		let record = Uuid::new_v4();
		let (mut plane, mut changes, _feed) = plane_showing(&[(record, vec![Uuid::new_v4()])]);
		plane.expecting.insert(record);

		plane.handle_files_changed(vec![(record, Vec::new())]);
		assert_eq!(
			changes.try_recv().expect("a change is sent"),
			FolderChange::Tags(vec![RecordTags {
				record,
				tags: Vec::new(),
			}])
		);
		assert!(plane.rereads.is_empty());
		assert!(plane.expecting.is_empty());
	}

	#[test]
	fn rows_for_records_outside_the_folder_are_ignored() {
		let (mut plane, mut changes, _feed) = plane_showing(&[(Uuid::new_v4(), Vec::new())]);
		plane.handle_files_changed(vec![(Uuid::new_v4(), vec![tag(Uuid::new_v4(), "Beach")])]);
		assert!(changes.try_recv().is_err());
		assert!(plane.rereads.is_empty());
	}

	#[test]
	fn tag_colors_parse_from_hex() {
		assert_eq!(parse_hex_color("#3b82f6"), Some(0x3b82f6));
		assert_eq!(parse_hex_color("7A3CE8"), Some(0x7a3ce8));
		assert_eq!(parse_hex_color("#fff"), None);
		assert_eq!(parse_hex_color("blue"), None);
	}

	#[test]
	fn a_file_outside_every_source_gets_its_own_words() {
		assert_eq!(
			tag_failure_message("no tracked source holds file 0190"),
			"Tags live in a source. Add this folder to your library first."
		);
		assert_eq!(
			tag_failure_message("unknown tags"),
			"Failed to toggle tag: unknown tags"
		);
	}

	/// A search started in the followed folder is listed in place of the
	/// folder, though the path has not moved.
	#[test]
	fn a_search_is_followed_in_place_of_the_folder() {
		let mut plane = plane_at(Some("laptop"));
		plane.apply_focus(focus("default", "laptop", "/photos"));
		let folder = plane
			.first_page("laptop")
			.expect("a folder has a first page");
		assert_eq!(
			folder.scope,
			SearchScope::Path {
				path: SdPath::Physical {
					device_slug: "laptop".to_string(),
					path: PathBuf::from("/photos"),
				},
			}
		);
		assert_eq!(plane.title(), "photos");

		let search = FileSearchInput::simple("beach".to_string());
		plane.apply_focus(NavigationFocus {
			search: Some(search.clone()),
			..focus("default", "laptop", "/photos")
		});
		assert_eq!(plane.search.as_ref(), Some(&search));
		let page = plane
			.first_page("laptop")
			.expect("a search has a first page");
		assert_eq!(
			(page.query, page.scope, page.filters, page.after, page.limit),
			(search.query, search.scope, search.filters, None, PAGE)
		);
		assert_eq!(plane.title(), "“beach”");

		plane.apply_focus(NavigationFocus {
			search: Some(FileSearchInput::simple(String::new())),
			..focus("default", "laptop", "/photos")
		});
		assert_eq!(plane.title(), "Search");
	}

	/// A later page lands on the end of the listing: the grid learns the new
	/// length and the cells, and a record on the new page takes tag changes.
	#[test]
	fn a_later_page_grows_the_listing() {
		let (mut plane, mut changes, feed) = plane_showing(&[(Uuid::new_v4(), Vec::new())]);
		let later = Uuid::new_v4();
		let beach = tag(Uuid::new_v4(), "Beach");

		plane.grow_listing(vec![Media {
			path: PathBuf::from("/photos/b.jpg"),
			record: later,
			tags: Vec::new(),
		}]);
		assert!(matches!(feed.try_recv(), Ok(Feed::Grew(2))));
		assert_eq!(
			changes.try_recv().expect("the page is sent"),
			FolderChange::Appended {
				records: vec![later],
				paths: vec![PathBuf::from("/photos/b.jpg")],
				tags: vec![Vec::new()],
			}
		);
		assert_eq!(plane.folder_state, FolderState::Ready(2));

		plane.handle_files_changed(vec![(later, vec![beach.clone()])]);
		assert_eq!(
			changes.try_recv().expect("a change is sent"),
			FolderChange::Tags(vec![RecordTags {
				record: later,
				tags: vec![beach.id],
			}])
		);
	}
}
