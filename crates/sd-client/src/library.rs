//! Current-library state shared across a client application.
//!
//! Library-scoped daemon ops need a library id on every call. Instead of
//! threading that id through every view, a [`LibraryContext`] holds the
//! current selection once: UIs read it, watch it for changes, and route
//! queries and actions through it so the id is injected automatically. The
//! selection persists to a JSON file so it survives restarts.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

use sd_core::infra::wire::Wire;

use crate::client::CoreClient;

/// On-disk shape of the persisted selection.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
	library_id: Option<Uuid>,
}

/// Shared handle to the current library selection.
///
/// Cloning is cheap; clones share the same state, watch channel, and state
/// file.
#[derive(Clone)]
pub struct LibraryContext {
	inner: Arc<Inner>,
}

struct Inner {
	client: CoreClient,
	state_path: PathBuf,
	current: watch::Sender<Option<Uuid>>,
	/// Serializes concurrent `set_current` calls so the state file always
	/// reflects the latest accepted selection.
	update_lock: Mutex<()>,
}

impl LibraryContext {
	/// Create a context backed by the state file at `state_path`, loading any
	/// previously persisted selection.
	///
	/// A missing file starts with no selection; an unreadable or malformed
	/// file is an error so callers can decide how to recover.
	pub async fn load(client: CoreClient, state_path: impl Into<PathBuf>) -> Result<Self> {
		let state_path = state_path.into();
		let initial = match tokio::fs::read(&state_path).await {
			Ok(bytes) => {
				serde_json::from_slice::<PersistedState>(&bytes)
					.with_context(|| {
						format!("invalid library state file at {}", state_path.display())
					})?
					.library_id
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
			Err(e) => {
				return Err(e).with_context(|| {
					format!(
						"failed to read library state file at {}",
						state_path.display()
					)
				});
			}
		};
		let (current, _) = watch::channel(initial);
		Ok(Self {
			inner: Arc::new(Inner {
				client,
				state_path,
				current,
				update_lock: Mutex::new(()),
			}),
		})
	}

	/// The currently selected library, if any.
	pub fn current(&self) -> Option<Uuid> {
		*self.inner.current.borrow()
	}

	/// Watch the selection; receivers are notified on every change.
	pub fn watch(&self) -> watch::Receiver<Option<Uuid>> {
		self.inner.current.subscribe()
	}

	/// Change the selection, notifying watchers and persisting to disk.
	///
	/// A no-op when the selection is unchanged. If persisting fails the
	/// in-memory selection (and watcher notification) still stands; the error
	/// reports only that the change will not survive a restart.
	pub async fn set_current(&self, library_id: Option<Uuid>) -> Result<()> {
		let _guard = self.inner.update_lock.lock().await;
		let changed = self.inner.current.send_if_modified(|current| {
			if *current == library_id {
				false
			} else {
				*current = library_id;
				true
			}
		});
		if !changed {
			return Ok(());
		}
		self.persist(library_id).await
	}

	/// The underlying client, for ops that are not library-scoped.
	pub fn client(&self) -> &CoreClient {
		&self.inner.client
	}

	/// Run a query with the current library id injected.
	pub async fn query<Q, O>(&self, query: &Q) -> Result<O>
	where
		Q: Wire + Serialize,
		O: DeserializeOwned,
	{
		self.inner.client.query(query, self.current()).await
	}

	/// Run an action with the current library id injected.
	pub async fn action<A>(&self, action: &A) -> Result<serde_json::Value>
	where
		A: Wire + Serialize,
	{
		self.inner.client.action(action, self.current()).await
	}

	/// Ask whether and how an action would run, with the current library id
	/// injected.
	pub async fn validate<A>(
		&self,
		action: &A,
	) -> Result<sd_core::infra::action::preflight::Validation>
	where
		A: Wire + Serialize,
	{
		self.inner.client.validate(action, self.current()).await
	}

	/// Ask what would exist after an action, with the current library id
	/// injected.
	pub async fn preview<A, P>(&self, action: &A) -> Result<P>
	where
		A: Wire + Serialize,
		P: DeserializeOwned,
	{
		self.inner.client.preview(action, self.current()).await
	}

	/// Write the selection to the state file via a temp-file rename so a
	/// crash mid-write cannot leave a truncated file behind.
	async fn persist(&self, library_id: Option<Uuid>) -> Result<()> {
		let path = &self.inner.state_path;
		if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
			tokio::fs::create_dir_all(parent).await.with_context(|| {
				format!("failed to create state directory {}", parent.display())
			})?;
		}
		let json = serde_json::to_vec_pretty(&PersistedState { library_id })
			.context("failed to serialize library state")?;
		let file_name = path
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_else(|| "library-state".to_string());
		let tmp = path.with_file_name(format!("{file_name}.tmp"));
		tokio::fs::write(&tmp, &json)
			.await
			.with_context(|| format!("failed to write library state to {}", tmp.display()))?;
		tokio::fs::rename(&tmp, path)
			.await
			.with_context(|| format!("failed to move library state into {}", path.display()))
	}
}
