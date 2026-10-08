//! The operations a guest asks the host for through `spacedrive_op`.
//!
//! Every operation that returns data to a guest lands here, named by a
//! string and carried as bytes. Requests are answered on the job's async
//! side, one at a time, because the guest blocks on each; so the state an
//! operation needs lives on [`JobOps`] for the job's duration and no
//! operation ever contends with another from the same job.
//!
//! Permission is checked here against the manifest, never in the guest.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sd_store::read::Start;
use sd_store::{FsEntry, SourceDb};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::sidecar::{SidecarFormat, SidecarKind, SidecarVariant};
use crate::domain::Tag;
use crate::infra::action::{error::ActionError, LibraryAction};
use crate::infra::job::prelude::JobContext;
use crate::library::Library;
use crate::ops::indexing::store::SourceStore;
use crate::ops::tags::{
	definitions, ApplyTagsAction, ApplyTagsInput, CreateTagAction, CreateTagInput, TagTargets,
	UnapplyTagsAction, UnapplyTagsInput,
};

use super::model_registry::{open_extension_store, ExtensionModelRegistry, CUSTOM_FIELD_MODEL};
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
	/// The directory the extension was installed from, for `config.json`.
	plugin_path: std::path::PathBuf,
	models: Arc<ExtensionModelRegistry>,
	/// The extension's store in this library, opened on the first model
	/// operation and closed when the job ends.
	store: Option<SourceDb>,
	task: Option<ActiveTask>,
}

#[derive(Deserialize)]
struct ModelRef {
	model: String,
	uuid: Option<Uuid>,
	content_uuid: Option<Uuid>,
}

#[derive(Deserialize)]
struct ModelPut {
	model: String,
	uuid: Option<Uuid>,
	content_uuid: Option<Uuid>,
	data: serde_json::Value,
}

#[derive(Deserialize)]
struct Inference {
	model: String,
	task: String,
}

#[derive(Deserialize)]
struct ModelList {
	model: String,
	limit: Option<usize>,
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
struct SidecarRef {
	content_uuid: Uuid,
	kind: String,
}

#[derive(Deserialize)]
struct SidecarWrite {
	content_uuid: Uuid,
	kind: String,
	data: serde_json::Value,
}

#[derive(Deserialize)]
struct FieldRef {
	record_uuid: Uuid,
	namespace: String,
	name: String,
}

#[derive(Deserialize)]
struct FieldSet {
	record_uuid: Uuid,
	namespace: String,
	name: String,
	value: serde_json::Value,
}

#[derive(Deserialize)]
struct JobDispatch {
	job: String,
	#[serde(default)]
	state: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct TagChange {
	record_uuid: Option<Uuid>,
	content_uuid: Option<Uuid>,
	tag: String,
}

/// A tag as the SDK's `Tag` deserializes it.
#[derive(Serialize)]
struct TagOut {
	id: Uuid,
	path: String,
	name: String,
}

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
		plugin_path: std::path::PathBuf,
		models: Arc<ExtensionModelRegistry>,
	) -> Self {
		Self {
			extension_id,
			manifest,
			library,
			plugin_path,
			models,
			store: None,
			task: None,
		}
	}

	/// Release what the job held open.
	pub async fn finish(self) {
		if let Some(store) = self.store {
			store.pool().close().await;
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
			"sidecars.exists" => self.sidecar_exists(parse(payload)?).await,
			"sidecars.read" => self.sidecar_read(parse(payload)?).await,
			"sidecars.write" => self.sidecar_write(parse(payload)?).await,
			"models.put" => self.model_put(parse(payload)?).await,
			"models.get" => self.model_get(parse(payload)?).await,
			"models.list" => self.model_list(parse(payload)?).await,
			"records.set_field" => self.field_set(parse(payload)?).await,
			"records.get_field" => self.field_get(parse(payload)?).await,
			"jobs.dispatch" => self.job_dispatch(parse(payload)?).await,
			"tags.add" => self.tag_change(parse(payload)?, true).await,
			"tags.remove" => self.tag_change(parse(payload)?, false).await,
			"ai.infer" => self.ai_infer(payload),
			"config.get" => self.config_get().await,
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
		// A tag filter is applied after the walk: tagged records are few
		// and the walk is what knows scope and extension.
		let tagged = match &query.tag {
			Some(tag) => Some(self.records_with_tag(tag).await?),
			None => None,
		};

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
				if tagged.is_some() {
					QUERY_CAP
				} else {
					limit - records.len()
				},
			)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
			records.extend(
				entries
					.into_iter()
					.filter(|entry| tagged.as_ref().is_none_or(|t| t.contains(&entry.uuid)))
					.take(limit - records.len())
					.map(|entry| RecordOut::new(store.id(), entry)),
			);
		}
		json(&records)
	}
}

impl JobOps {
	/// Where a sidecar of one of this extension's kinds lives: the library's
	/// content-addressed sidecar tree, under the extension's namespace. The
	/// SDK's sidecars are JSON documents with one variant.
	fn sidecar_path(
		&self,
		content_uuid: Uuid,
		kind: &str,
		write: bool,
	) -> Result<std::path::PathBuf, OpError> {
		let permissions = &self.manifest.permissions;
		let granted = if write {
			permissions.can_write_sidecar(kind)
		} else {
			permissions.can_read_sidecar(kind)
		};
		if !granted {
			return Err(OpError::permission_denied(format!(
				"{} may not {} sidecar kind {kind}",
				self.extension_id,
				if write { "write" } else { "read" }
			)));
		}
		let kind =
			SidecarKind::extension(&self.extension_id, kind).map_err(OpError::invalid_input)?;
		Ok(self
			.library
			.path()
			.join("sidecars")
			.join(sd_sidecar_path::relative_path(
				&content_uuid,
				&kind.directory(),
				SidecarVariant::new("default").as_str(),
				SidecarFormat::Json.extension(),
			)))
	}

	async fn sidecar_exists(&self, sidecar: SidecarRef) -> OpResult {
		let path = self.sidecar_path(sidecar.content_uuid, &sidecar.kind, false)?;
		json(&tokio::fs::try_exists(&path).await.unwrap_or(false))
	}

	async fn sidecar_read(&self, sidecar: SidecarRef) -> OpResult {
		let path = self.sidecar_path(sidecar.content_uuid, &sidecar.kind, false)?;
		tokio::fs::read(&path).await.map_err(|e| match e.kind() {
			std::io::ErrorKind::NotFound => OpError::not_found(),
			_ => OpError::failed(format!("read {}: {e}", path.display())),
		})
	}

	/// Written whole and renamed into place, so a reader never sees a
	/// partial document and a crash leaves either the old one or the new.
	async fn sidecar_write(&self, sidecar: SidecarWrite) -> OpResult {
		let path = self.sidecar_path(sidecar.content_uuid, &sidecar.kind, true)?;
		let bytes =
			serde_json::to_vec(&sidecar.data).map_err(|e| OpError::failed(e.to_string()))?;
		let io = |e: std::io::Error| OpError::failed(format!("write {}: {e}", path.display()));
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await.map_err(io)?;
		}
		let tmp = path.with_extension("json.tmp");
		tokio::fs::write(&tmp, &bytes).await.map_err(io)?;
		tokio::fs::rename(&tmp, &path).await.map_err(io)?;
		json(&serde_json::Value::Null)
	}
}

impl JobOps {
	/// The extension's store in this library, opened on first use.
	async fn store(&mut self) -> Result<&SourceDb, OpError> {
		if self.store.is_none() {
			let schema = self.models.schema_for(&self.extension_id);
			let db = open_extension_store(self.library.path(), &self.extension_id, &schema)
				.await
				.map_err(|e| OpError::failed(format!("open extension store: {e}")))?;
			self.store = Some(db);
		}
		Ok(self.store.as_ref().expect("opened above"))
	}

	/// A standalone model is keyed by its own uuid, a content-scoped one by
	/// the content it describes. Both are the record's external id.
	fn model_key(uuid: Option<Uuid>, content_uuid: Option<Uuid>) -> Result<String, OpError> {
		match (uuid, content_uuid) {
			(Some(uuid), None) => Ok(uuid.to_string()),
			(None, Some(content)) => Ok(format!("content:{content}")),
			_ => Err(OpError::invalid_input(
				"a model is keyed by exactly one of uuid or content_uuid",
			)),
		}
	}

	async fn model_put(&mut self, put: ModelPut) -> OpResult {
		let key = Self::model_key(put.uuid, put.content_uuid)?;
		let store = self.store().await?;
		store
			.upsert(&put.model, &key, &put.data)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
		json(&serde_json::Value::Null)
	}

	async fn model_get(&mut self, model: ModelRef) -> OpResult {
		let key = Self::model_key(model.uuid, model.content_uuid)?;
		let store = self.store().await?;
		let rows = store
			.facet_rows(&model.model, Some(&key), 1)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
		match rows.into_iter().next() {
			Some(row) => json(&row),
			None => Err(OpError::not_found()),
		}
	}

	async fn model_list(&mut self, list: ModelList) -> OpResult {
		let store = self.store().await?;
		let rows = store
			.facet_rows(
				&list.model,
				None,
				list.limit.unwrap_or(QUERY_CAP).min(QUERY_CAP),
			)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
		json(&rows)
	}
}

impl JobOps {
	/// A field name becomes part of the row key, so it is kept to one
	/// identifier-like token.
	fn field_key(record: Uuid, namespace: &str, name: &str) -> Result<String, OpError> {
		let token = |s: &str| {
			!s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
		};
		if !token(namespace) || !token(name) {
			return Err(OpError::invalid_input(format!(
				"custom field {namespace}.{name}: namespace and name are [A-Za-z0-9_]"
			)));
		}
		Ok(format!("{record}:{namespace}:{name}"))
	}

	/// Set one custom field on a record, under a namespace the manifest's
	/// `write_custom_fields` grants. The record itself is not checked: the
	/// field lives in the extension's own store and names the record by
	/// uuid, so a field on a record that later vanishes is just an orphan
	/// row.
	async fn field_set(&mut self, set: FieldSet) -> OpResult {
		if !self
			.manifest
			.permissions
			.write_custom_fields
			.iter()
			.any(|n| *n == set.namespace)
		{
			return Err(OpError::permission_denied(format!(
				"{} has no write_custom_fields grant for {}",
				self.extension_id, set.namespace
			)));
		}
		let key = Self::field_key(set.record_uuid, &set.namespace, &set.name)?;
		let row = serde_json::json!({
			"record": set.record_uuid.to_string(),
			"namespace": set.namespace,
			"name": set.name,
			"value": set.value.to_string(),
		});
		let store = self.store().await?;
		store
			.upsert(CUSTOM_FIELD_MODEL, &key, &row)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
		json(&serde_json::Value::Null)
	}

	/// One custom field's value, or `null` when the record has none. Fields
	/// are the extension's own, so reading needs no grant.
	async fn field_get(&mut self, field: FieldRef) -> OpResult {
		let key = Self::field_key(field.record_uuid, &field.namespace, &field.name)?;
		let store = self.store().await?;
		let rows = store
			.facet_rows(CUSTOM_FIELD_MODEL, Some(&key), 1)
			.await
			.map_err(|e| OpError::failed(e.to_string()))?;
		let value = rows
			.into_iter()
			.next()
			.and_then(|row| row["value"].as_str().map(str::to_string))
			.map(|text| serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
			.unwrap_or(serde_json::Value::Null);
		json(&value)
	}
}

impl JobOps {
	/// Queue another of this extension's jobs, with its starting state.
	///
	/// The name is resolved under the caller's own extension id, so an
	/// extension can never start another extension's job. The new job runs
	/// through the library's job manager like one started from the API; it
	/// waits for the plugin's runtime once the caller's job has released it.
	async fn job_dispatch(&self, dispatch: JobDispatch) -> OpResult {
		if !self.manifest.permissions.dispatch_jobs {
			return Err(OpError::permission_denied(format!(
				"{} has no dispatch_jobs grant",
				self.extension_id
			)));
		}
		if dispatch.job.contains(':') {
			return Err(OpError::invalid_input(format!(
				"job {:?}: name the job as the extension registered it, without an extension id",
				dispatch.job
			)));
		}
		let plugin_manager = self
			.library
			.core_context()
			.get_plugin_manager()
			.await
			.ok_or_else(|| OpError::failed("extensions are not initialized"))?;
		let job = plugin_manager
			.read()
			.await
			.job_registry()
			.create_wasm_job(
				&format!("{}:{}", self.extension_id, dispatch.job),
				dispatch.state.map(|s| s.to_string()).unwrap_or_default(),
			)
			.map_err(OpError::invalid_input)?;
		let handle = self
			.library
			.jobs()
			.dispatch(job)
			.await
			.map_err(|e| OpError::failed(format!("dispatch {}: {e}", dispatch.job)))?;
		json(&serde_json::json!({ "job_id": handle.id().0 }))
	}
}

impl JobOps {
	/// Add or remove one tag, named by path, on one record or on one
	/// content identity.
	///
	/// A tag that does not exist yet is created by name, through the same
	/// `tags.create` path a person uses, so a tag an extension coins is an
	/// ordinary tag: it lands in the staging table, travels with its first
	/// assertion and is found by slug the next time anything names it.
	async fn tag_change(&self, change: TagChange, add: bool) -> OpResult {
		if !self.manifest.permissions.write_tags {
			return Err(OpError::permission_denied(format!(
				"{} has no write_tags grant",
				self.extension_id
			)));
		}
		let targets = match (change.record_uuid, change.content_uuid) {
			(Some(record), None) => TagTargets::File(vec![record]),
			(None, Some(content)) => TagTargets::Content(vec![content]),
			_ => {
				return Err(OpError::invalid_input(
					"a tag change names exactly one of record_uuid or content_uuid",
				))
			}
		};
		let context = self.library.core_context().clone();
		let action_error = |e: ActionError| match e {
			ActionError::InvalidInput(message) => OpError::invalid_input(message),
			other => OpError::failed(other.to_string()),
		};

		let tag = if add {
			CreateTagAction::from_input(CreateTagInput {
				path: change.tag.clone(),
				color: None,
				icon: None,
			})
			.map_err(OpError::invalid_input)?
			.execute(self.library.clone(), context.clone())
			.await
			.map_err(action_error)?
			.tag
		} else {
			// Removing a tag that was never defined is a no-op, not a reason
			// to mint a definition nothing carries.
			let path = sd_store::normalize_tag_path(&change.tag)
				.map_err(|e| OpError::invalid_input(e.to_string()))?;
			let Some(definition) = definitions::find_by_slug(
				&self.library,
				context.volume_index(),
				sd_store::slug_for_path(&path),
			)
			.await
			else {
				return json(&serde_json::Value::Null);
			};
			Tag::from_definition(&definition)
		};

		let result = if add {
			ApplyTagsAction::from_input(ApplyTagsInput {
				targets,
				tag_ids: vec![tag.id],
			})
			.map_err(OpError::invalid_input)?
			.execute(self.library.clone(), context)
			.await
			.map(|_| ())
		} else {
			UnapplyTagsAction::from_input(UnapplyTagsInput {
				targets,
				tag_ids: vec![tag.id],
			})
			.map_err(OpError::invalid_input)?
			.execute(self.library.clone(), context)
			.await
			.map(|_| ())
		};
		result.map_err(action_error)?;
		json(&TagOut {
			id: tag.id,
			path: tag.path,
			name: tag.name,
		})
	}

	/// Records in the library's stores that carry a tag, by path. A tag
	/// nobody defined names no records.
	async fn records_with_tag(&self, tag: &str) -> Result<HashSet<Uuid>, OpError> {
		let path =
			sd_store::normalize_tag_path(tag).map_err(|e| OpError::invalid_input(e.to_string()))?;
		let index = self.library.core_context().volume_index();
		let Some(definition) =
			definitions::find_by_slug(&self.library, index, sd_store::slug_for_path(&path)).await
		else {
			return Ok(HashSet::new());
		};
		let mut records = HashSet::new();
		for store in self.stores().await {
			match store.db().records_with_tag(definition.uuid).await {
				Ok(found) => records.extend(found),
				Err(error) => {
					tracing::warn!(source = %store.id(), %error, "tagged records unavailable")
				}
			}
		}
		Ok(records)
	}
}

impl JobOps {
	/// Inference, when the core has a provider for the model's category.
	///
	/// It has none for the categories photos asks for (face detection, scene
	/// classification, embeddings, language models), so a granted request
	/// is answered `not_available` with the category named, and the job
	/// decides what to do without it. The grant is still checked first: a
	/// request for an undeclared category is refused, not deferred.
	fn ai_infer(&self, payload: &[u8]) -> OpResult {
		let (header, _input) = split_frame(payload)?;
		let inference: Inference = parse(header)?;
		let category = inference
			.model
			.split_once(':')
			.map(|(category, _)| category)
			.unwrap_or(&inference.model);
		let granted = self
			.manifest
			.permissions
			.use_models
			.iter()
			.any(|m| m.category == category);
		if !granted {
			return Err(OpError::permission_denied(format!(
				"{} has no use_models grant for {category}",
				self.extension_id
			)));
		}
		Err(OpError::not_available(format!(
			"no {category} provider is installed for {} ({})",
			inference.model, inference.task
		)))
	}

	/// `config.json` beside the manifest, or `{}`.
	async fn config_get(&self) -> OpResult {
		match tokio::fs::read(self.plugin_path.join("config.json")).await {
			Ok(bytes) => {
				let value: serde_json::Value = serde_json::from_slice(&bytes)
					.map_err(|e| OpError::failed(format!("config.json: {e}")))?;
				json(&value)
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => json(&serde_json::json!({})),
			Err(e) => Err(OpError::failed(format!("config.json: {e}"))),
		}
	}
}

/// A framed payload: a little-endian u32 header length, the JSON header,
/// then raw bytes.
fn split_frame(payload: &[u8]) -> Result<(&[u8], &[u8]), OpError> {
	let malformed = || OpError::invalid_input("malformed framed payload");
	let len = payload
		.get(..4)
		.and_then(|b| <[u8; 4]>::try_from(b).ok())
		.map(u32::from_le_bytes)
		.ok_or_else(malformed)? as usize;
	let rest = &payload[4..];
	if rest.len() < len {
		return Err(malformed());
	}
	Ok(rest.split_at(len))
}

fn parse<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Result<T, OpError> {
	serde_json::from_slice(payload).map_err(|e| OpError::invalid_input(e.to_string()))
}

fn json<T: Serialize>(value: &T) -> OpResult {
	serde_json::to_vec(value).map_err(|e| OpError::failed(e.to_string()))
}
