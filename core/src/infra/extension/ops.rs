//! The operations a guest asks the host for through `spacedrive_op`.
//!
//! Every operation that returns data to a guest lands here, named by a
//! string and carried as bytes. Requests are answered on the job's async
//! side, one at a time, because the guest blocks on each; so the state an
//! operation needs lives on [`JobOps`] for the job's duration and no
//! operation ever contends with another from the same job.
//!
//! Permission is checked here against the manifest, never in the guest.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sd_store::read::Start;
use sd_store::FsEntry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::infra::job::prelude::JobContext;
use crate::library::Library;
use crate::ops::indexing::store::SourceStore;

use super::types::ExtensionManifest;

/// An error a guest can act on. `code` is the stable part: the SDK maps it
/// onto its own error type, and `message` is for the log.
#[derive(Debug, Clone, Serialize)]
pub struct OpError {
	pub code: &'static str,
	pub message: String,
}

impl OpError {
	pub fn not_found() -> Self {
		Self::new("not_found", "not found")
	}

	pub fn permission_denied(message: impl Into<String>) -> Self {
		Self::new("permission_denied", message)
	}

	pub fn invalid_input(message: impl Into<String>) -> Self {
		Self::new("invalid_input", message)
	}

	pub fn not_available(message: impl Into<String>) -> Self {
		Self::new("not_available", message)
	}

	pub fn failed(message: impl Into<String>) -> Self {
		Self::new("failed", message)
	}

	fn new(code: &'static str, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
		}
	}

	pub fn to_json(&self) -> String {
		serde_json::to_string(self)
			.unwrap_or_else(|_| format!(r#"{{"code":"{}","message":"unserializable"}}"#, self.code))
	}
}

pub type OpResult = Result<Vec<u8>, OpError>;

/// A task attempt the guest is inside, as `task.begin` declared it.
struct ActiveTask {
	name: String,
	attempt: u32,
	started: Instant,
	deadline: Option<Instant>,
}

/// The host side of one running extension job.
pub struct JobOps {
	pub extension_id: String,
	pub manifest: Arc<ExtensionManifest>,
	pub library: Arc<Library>,
	task: Option<ActiveTask>,
}

/// A record as the SDK's `Record` deserializes it.
#[derive(Serialize)]
struct RecordOut {
	uuid: Uuid,
	source_id: Uuid,
	name: String,
	kind: &'static str,
	extension: Option<String>,
	relative_path: String,
	size: Option<u64>,
	modified_ms: Option<i64>,
	content_uuid: Option<Uuid>,
}

impl RecordOut {
	fn new(source_id: Uuid, entry: FsEntry) -> Self {
		Self {
			uuid: entry.uuid,
			source_id,
			name: entry.name,
			kind: entry.kind.as_str(),
			extension: entry.extension.map(|e| e.to_lowercase()),
			relative_path: entry.relative_path,
			size: entry.size.and_then(|s| u64::try_from(s).ok()),
			modified_ms: entry.mtime_ms,
			content_uuid: entry.content_uuid,
		}
	}
}

#[derive(Deserialize)]
struct RecordRef {
	uuid: Uuid,
}

#[derive(Deserialize)]
struct RecordQuery {
	source: Option<Uuid>,
	scope: Option<String>,
	extensions: Option<Vec<String>>,
	tag: Option<String>,
	limit: Option<usize>,
}

/// The most records one query answers with.
const QUERY_CAP: usize = 10_000;

#[derive(Deserialize)]
struct TaskBegin {
	name: String,
	attempt: u32,
	retries: u32,
	timeout_ms: u64,
}

#[derive(Deserialize)]
struct TaskEnd {
	name: String,
	attempt: u32,
	error: Option<String>,
}

impl JobOps {
	pub fn new(
		extension_id: String,
		manifest: Arc<ExtensionManifest>,
		library: Arc<Library>,
	) -> Self {
		Self {
			extension_id,
			manifest,
			library,
			task: None,
		}
	}

	/// Answer one request.
	///
	/// Once the active task's deadline has passed, every operation but the
	/// one that ends the task fails with `timeout`, which is how a
	/// single-threaded guest gets stopped mid-task: it cannot be preempted,
	/// but it cannot make progress without the host either.
	pub async fn handle(&mut self, op: &str, payload: &[u8], ctx: &JobContext<'_>) -> OpResult {
		if op != "task.end" {
			if let Some(task) = &self.task {
				if task
					.deadline
					.is_some_and(|deadline| Instant::now() >= deadline)
				{
					return Err(OpError::new(
						"timeout",
						format!("task {} ran past its deadline", task.name),
					));
				}
			}
		}

		match op {
			"task.begin" => self.task_begin(parse(payload)?, ctx),
			"task.end" => self.task_end(parse(payload)?, ctx),
			"records.get" => self.record_get(parse(payload)?).await,
			"records.read" => self.record_read(parse(payload)?).await,
			"records.query" => self.record_query(parse(payload)?).await,
			_ => Err(OpError::new(
				"unknown_op",
				format!("unknown operation {op}"),
			)),
		}
	}

	fn task_begin(&mut self, begin: TaskBegin, ctx: &JobContext<'_>) -> OpResult {
		if let Some(active) = &self.task {
			return Err(OpError::invalid_input(format!(
				"task {} started while task {} is running",
				begin.name, active.name
			)));
		}
		ctx.log(format!(
			"task {} attempt {}/{} started",
			begin.name,
			begin.attempt + 1,
			begin.retries + 1
		));
		let started = Instant::now();
		self.task = Some(ActiveTask {
			name: begin.name,
			attempt: begin.attempt,
			started,
			deadline: (begin.timeout_ms > 0)
				.then(|| started + Duration::from_millis(begin.timeout_ms)),
		});
		json(&serde_json::json!({ "id": begin.attempt }))
	}

	fn task_end(&mut self, end: TaskEnd, ctx: &JobContext<'_>) -> OpResult {
		let Some(task) = self.task.take() else {
			return Err(OpError::invalid_input(format!(
				"task {} ended without starting",
				end.name
			)));
		};
		let elapsed = task.started.elapsed().as_millis();
		match end.error {
			None => ctx.log(format!(
				"task {} attempt {} finished in {elapsed} ms",
				task.name,
				task.attempt + 1
			)),
			Some(error) => ctx.log(format!(
				"task {} attempt {} failed after {elapsed} ms: {error}",
				task.name,
				task.attempt + 1
			)),
		}
		json(&serde_json::Value::Null)
	}
}

impl JobOps {
	/// The stores of this library's sources, detached ones included: their
	/// records are still true even when their bytes are out of reach.
	async fn stores(&self) -> Vec<Arc<SourceStore>> {
		let index = self.library.core_context().volume_index();
		let mut stores = Vec::new();
		for source in index.sources_of(self.library.id()) {
			if let Some(store) = index.store_for(&source.root).await {
				stores.push(store);
			}
		}
		stores
	}

	/// The store holding a record, with the entry.
	async fn locate(&self, uuid: Uuid) -> Result<(Arc<SourceStore>, FsEntry), OpError> {
		for store in self.stores().await {
			match store.db().entry_by_uuid(uuid).await {
				Ok(Some(entry)) => return Ok((store, entry)),
				Ok(None) => {}
				Err(error) => {
					tracing::warn!(source = %store.id(), %error, "record lookup failed")
				}
			}
		}
		Err(OpError::not_found())
	}

	fn check_read(&self, entry: &FsEntry) -> Result<(), OpError> {
		let extension = entry.extension.as_deref().map(str::to_lowercase);
		if self
			.manifest
			.permissions
			.can_read_record(extension.as_deref())
		{
			return Ok(());
		}
		Err(OpError::permission_denied(format!(
			"{} may not read {}",
			self.extension_id, entry.relative_path
		)))
	}

	async fn record_get(&self, record: RecordRef) -> OpResult {
		let (store, entry) = self.locate(record.uuid).await?;
		self.check_read(&entry)?;
		json(&RecordOut::new(store.id(), entry))
	}

	/// A record's bytes, through the path its source resolves. A detached
	/// source answers not found: the record is still known, the bytes are
	/// not here.
	async fn record_read(&self, record: RecordRef) -> OpResult {
		let (store, entry) = self.locate(record.uuid).await?;
		self.check_read(&entry)?;
		let path = store.root().join(&entry.relative_path);
		tokio::fs::read(&path).await.map_err(|e| match e.kind() {
			std::io::ErrorKind::NotFound => OpError::not_found(),
			_ => OpError::failed(format!("read {}: {e}", path.display())),
		})
	}

	/// Files across the library's stores, narrowed by the query and by the
	/// grant: a glob grant restricts the extensions a query without its own
	/// list gets back.
	async fn record_query(&self, query: RecordQuery) -> OpResult {
		if query.tag.is_some() {
			return Err(OpError::invalid_input(
				"record queries cannot filter by tag yet",
			));
		}
		let permissions = &self.manifest.permissions;
		if permissions.read_records.is_none() {
			return Err(OpError::permission_denied(format!(
				"{} has no read_records grant",
				self.extension_id
			)));
		}
		let extensions: Option<Vec<String>> = match query.extensions {
			Some(asked) => Some(
				asked
					.into_iter()
					.filter(|e| permissions.can_read_record(Some(e)))
					.collect(),
			),
			None => permissions.granted_extensions(),
		};
		let limit = query.limit.unwrap_or(QUERY_CAP).min(QUERY_CAP);
		let scope = query.scope.unwrap_or_default();

		let mut records = Vec::new();
		for store in self.stores().await {
			if query.source.is_some_and(|source| source != store.id()) {
				continue;
			}
			if records.len() >= limit {
				break;
			}
			let entries = sd_store::read::files_beneath(
				store.db().pool(),
				&scope,
				Start::First,
				extensions.as_deref(),
				false,
				limit - records.len(),
			)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
			records.extend(
				entries
					.into_iter()
					.map(|entry| RecordOut::new(store.id(), entry)),
			);
		}
		json(&records)
	}
}

fn parse<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, OpError> {
	serde_json::from_slice(payload).map_err(|e| OpError::invalid_input(e.to_string()))
}

fn json<T: Serialize>(value: &T) -> OpResult {
	serde_json::to_vec(value).map_err(|e| OpError::failed(e.to_string()))
}
