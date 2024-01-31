use futures::Future;
use std::{collections::HashMap, pin::Pin, sync::Arc};
use tokio::{
	sync::{broadcast, oneshot, Mutex},
	task::AbortHandle,
};

pub struct Actor {
	pub abort_handle: Mutex<Option<AbortHandle>>,
	pub spawn_fn: Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>,
}

pub struct Actors {
	pub invalidate_rx: broadcast::Receiver<()>,
	invalidate_tx: broadcast::Sender<()>,
	actors: Arc<Mutex<HashMap<String, Arc<Actor>>>>,
}

impl Actors {
	pub async fn declare<F: Future<Output = ()> + Send + 'static>(
		self: &Arc<Self>,
		name: &str,
		actor_fn: impl FnOnce() -> F + Send + Sync + Clone + 'static,
		autostart: bool,
	) {
		self.actors.lock().await.insert(
			name.to_string(),
			Arc::new(Actor {
				abort_handle: Default::default(),
				spawn_fn: Arc::new(move || Box::pin((actor_fn.clone())()) as Pin<Box<_>>),
			}),
		);

		if autostart {
			self.start(name).await;
		}
	}

	pub async fn start(self: &Arc<Self>, name: &str) {
		let name = name.to_string();
		let actors = self.actors.lock().await;

		let Some(actor) = actors.get(&name).cloned() else {
			return;
		};

		let mut abort_handle = actor.abort_handle.lock().await;
		if abort_handle.is_some() {
			return;
		}

		let (tx, rx) = oneshot::channel();

		let invalidate_tx = self.invalidate_tx.clone();

		let spawn_fn = actor.spawn_fn.clone();

		let task = tokio::spawn(async move {
			(spawn_fn)().await;

			tx.send(()).ok();
		});

		*abort_handle = Some(task.abort_handle());
		invalidate_tx.send(()).ok();

		tokio::spawn({
			let actor = actor.clone();
			async move {
				#[allow(clippy::match_single_binding)]
				match rx.await {
					_ => {}
				};

				actor.abort_handle.lock().await.take();
				invalidate_tx.send(()).ok();
			}
		});
	}

	pub async fn stop(self: &Arc<Self>, name: &str) {
		let name = name.to_string();
		let actors = self.actors.lock().await;

		let Some(actor) = actors.get(&name).cloned() else {
			return;
		};

		let mut abort_handle = actor.abort_handle.lock().await;

		if let Some(abort_handle) = abort_handle.take() {
			abort_handle.abort();
		}
	}

	pub async fn get_state(&self) -> HashMap<String, bool> {
		let actors = self.actors.lock().await;

		let mut state = HashMap::new();

		for (name, actor) in &*actors {
			state.insert(name.to_string(), actor.abort_handle.lock().await.is_some());
		}

		state
	}
}

impl Default for Actors {
	fn default() -> Self {
		let actors = Default::default();

		let (invalidate_tx, invalidate_rx) = broadcast::channel(1);

		Self {
			actors,
			invalidate_rx,
			invalidate_tx,
		}
	}
}
