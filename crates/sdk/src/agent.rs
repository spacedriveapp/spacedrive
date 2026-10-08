//! Agent context and memory systems

use crate::ai::{AiContext, AiModelRegistry};
use crate::types::*;
use crate::vdfs::VdfsContext;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

/// Agent execution context
pub struct AgentContext<M> {
	_phantom: PhantomData<M>,
}

impl<M: AgentMemory> AgentContext<M> {
	pub fn vdfs(&self) -> VdfsContext {
		VdfsContext
	}

	pub fn ai(&self) -> AiContext {
		AiContext
	}

	pub fn ai_models(&self) -> AiModelRegistry {
		AiModelRegistry
	}

	pub fn models(&self) -> AiModelRegistry {
		self.ai_models()
	}

	pub fn jobs(&self) -> JobDispatcher {
		JobDispatcher
	}

	pub fn memory(&self) -> MemoryHandle<M> {
		MemoryHandle::new()
	}

	pub fn trace(&self, _message: impl Into<String>) {
		// Log to agent trail
	}

	pub fn in_granted_scope(&self, _path: &str) -> bool {
		panic!("Check scope")
	}

	pub fn config<C>(&self) -> &C {
		panic!("Get config")
	}

	pub fn notify(&self) -> NotificationBuilder {
		NotificationBuilder::default()
	}
}

/// Queues this extension's own jobs. Needs the manifest's `dispatch_jobs`.
#[derive(Default)]
pub struct JobDispatcher;

impl JobDispatcher {
	/// A job by the name its `#[job(name = "...")]` registered, with the
	/// state it starts from. Only this extension's jobs can be named.
	pub fn dispatch<S: Serialize>(&self, job: &str, state: &S) -> JobDispatchBuilder {
		JobDispatchBuilder {
			job: job.to_string(),
			state: serde_json::to_value(state).map_err(|e| Error::Serialization(e.to_string())),
		}
	}
}

pub struct JobDispatchBuilder {
	job: String,
	state: Result<serde_json::Value>,
}

impl JobDispatchBuilder {
	/// Priority and placement are not carried to the host yet; the job runs
	/// at normal priority on this device.
	pub fn priority(self, _priority: Priority) -> Self {
		self
	}

	pub fn when_idle(self) -> Self {
		self
	}

	pub fn on_device_with_capability(self, _cap: Capability) -> Self {
		self
	}

	/// Queue the job and return its id. It runs once the job manager picks
	/// it up, which for a job dispatched from inside another of this
	/// extension's jobs is after the caller finishes.
	pub async fn execute(self) -> Result<Uuid> {
		#[derive(serde::Deserialize)]
		struct Dispatched {
			job_id: Uuid,
		}
		let dispatched: Dispatched = crate::ffi::op_json(
			"jobs.dispatch",
			&serde_json::json!({ "job": self.job, "state": self.state? }),
		)?;
		Ok(dispatched.job_id)
	}
}

#[derive(Default)]
pub struct NotificationBuilder;

impl NotificationBuilder {
	pub fn message(self, _msg: impl Into<String>) -> Self {
		self
	}

	pub fn on_active_device(self) -> Self {
		self
	}

	pub fn with_title(self, _title: impl Into<String>) -> Self {
		self
	}

	pub async fn send(self) -> Result<()> {
		panic!("Send notification")
	}
}

pub struct MemoryHandle<M> {
	_phantom: PhantomData<M>,
}

impl<M: AgentMemory> MemoryHandle<M> {
	pub fn new() -> Self {
		Self {
			_phantom: PhantomData,
		}
	}

	pub async fn read(&self) -> MemoryReadGuard<M> {
		panic!("Load memory")
	}

	pub async fn write(&self) -> MemoryWriteGuard<M> {
		panic!("Load for writing")
	}
}

pub struct MemoryReadGuard<M> {
	_phantom: PhantomData<M>,
}

impl<M> std::ops::Deref for MemoryReadGuard<M> {
	type Target = M;
	fn deref(&self) -> &Self::Target {
		panic!("not implemented")
	}
}

pub struct MemoryWriteGuard<M> {
	_phantom: PhantomData<M>,
}

impl<M> std::ops::Deref for MemoryWriteGuard<M> {
	type Target = M;
	fn deref(&self) -> &Self::Target {
		panic!("not implemented")
	}
}

impl<M> std::ops::DerefMut for MemoryWriteGuard<M> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		panic!("not implemented")
	}
}

pub trait AgentMemory: Send + Sync {}

/// Marker trait for enum variants used in memory queries
pub trait MemoryVariant {
	fn variant_name(&self) -> &'static str;
}

pub struct TemporalMemory<T> {
	_phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Clone> TemporalMemory<T> {
	pub async fn append(&mut self, _event: T) -> Result<()> {
		panic!("not implemented")
	}

	pub fn query(&self) -> TemporalQuery<T> {
		TemporalQuery {
			_phantom: PhantomData,
		}
	}
}

pub struct TemporalQuery<T> {
	_phantom: PhantomData<T>,
}

impl<T: Clone> TemporalQuery<T> {
	pub fn where_variant<V: MemoryVariant>(self, _variant: V) -> Self {
		self
	}

	pub fn since(self, _duration: chrono::Duration) -> Self {
		self
	}

	pub fn where_field(self, _field: &str, _pred: crate::vdfs::FieldPredicate) -> Self {
		self
	}

	pub fn where_semantic(self, _field: &str, _query: crate::vdfs::SemanticQuery) -> Self {
		self
	}

	pub fn sort_by<F>(self, _f: F) -> Self
	where
		F: Fn(&T, &T) -> std::cmp::Ordering,
	{
		self
	}

	pub fn sort_by_relevance(self) -> Self {
		self
	}

	pub fn limit(self, _n: usize) -> Self {
		self
	}

	pub async fn collect(self) -> Result<Vec<T>> {
		panic!("not implemented")
	}
}

pub struct AssociativeMemory<T> {
	_phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Clone> AssociativeMemory<T> {
	pub async fn add(&mut self, _knowledge: T) -> Result<()> {
		panic!("not implemented")
	}

	pub fn query(&self) -> AssociativeQuery<T> {
		AssociativeQuery {
			_phantom: PhantomData,
			query_text: None,
		}
	}

	pub fn query_similar(&self, query: &str) -> AssociativeQuery<T> {
		AssociativeQuery {
			_phantom: PhantomData,
			query_text: Some(query.to_string()),
		}
	}
}

pub struct AssociativeQuery<T> {
	_phantom: PhantomData<T>,
	query_text: Option<String>,
}

impl<T: Clone> AssociativeQuery<T> {
	pub fn where_variant<V: MemoryVariant>(self, _variant: V) -> Self {
		self
	}

	pub fn where_field(self, _field: &str, _pred: crate::vdfs::FieldPredicate) -> Self {
		self
	}

	pub fn min_similarity(self, _threshold: f32) -> Self {
		self
	}

	pub fn top_k(self, _k: usize) -> Self {
		self
	}

	pub fn within_context<U>(self, _context: &[U]) -> Self {
		self
	}

	pub fn and_related_concepts(self, _depth: usize) -> Self {
		self
	}

	pub async fn collect(self) -> Result<Vec<T>> {
		panic!("not implemented")
	}
}

pub struct WorkingMemory<T> {
	_phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Clone + Default> WorkingMemory<T> {
	pub async fn read(&self) -> T {
		panic!("not implemented")
	}

	pub async fn update<F>(&mut self, _f: F) -> Result<()>
	where
		F: FnOnce(T) -> Result<T>,
	{
		panic!("not implemented")
	}
}
