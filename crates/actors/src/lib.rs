use std::{
	collections::HashMap,
	future::{Future, IntoFuture},
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use async_channel as chan;
use tokio::{
	sync::{broadcast, oneshot, Mutex, RwLock},
	task::AbortHandle,
};
use tracing::{error, instrument, warn};

type ActorFn = dyn Fn(StopActor) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync;
type ActorsMap = HashMap<&'static str, (Arc<Actor>, ActorRunState)>;

pub struct Actor {
	abort_handle: Mutex<Option<AbortHandle>>,
	spawn_fn: Arc<ActorFn>,
	stop_tx: chan::Sender<()>,
	stop_rx: chan::Receiver<()>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorRunState {
	Running,
	Stopped,
}

pub struct Actors {
	pub invalidate_rx: broadcast::Receiver<()>,
	invalidate_tx: broadcast::Sender<()>,
	actors: Arc<RwLock<ActorsMap>>,
}

impl Actors {
	pub async fn declare<F: Future<Output = ()> + Send + 'static>(
		self: &Arc<Self>,
		name: &'static str,
		actor_fn: impl FnOnce(StopActor) -> F + Send + Sync + Clone + 'static,
		autostart: bool,
	) {
		let (stop_tx, stop_rx) = chan::bounded(1);

		self.actors.write().await.insert(
			name,
			(
				Arc::new(Actor {
					abort_handle: Default::default(),
					spawn_fn: Arc::new(move |stop| {
						Box::pin((actor_fn.clone())(stop)) as Pin<Box<_>>
					}),
					stop_tx,
					stop_rx,
				}),
				ActorRunState::Stopped,
			),
		);

		if autostart {
			self.start(name).await;
		}
	}

	#[instrument(skip(self))]
	pub async fn start(self: &Arc<Self>, name: &str) {
		let actor = {
			let mut actors = self.actors.write().await;

			let Some((actor, run_state)) = actors.get_mut(name) else {
				return;
			};

			if matches!(run_state, ActorRunState::Running) {
				warn!("Actor already running!");
				return;
			}

			*run_state = ActorRunState::Running;

			Arc::clone(actor)
		};

		let mut abort_handle = actor.abort_handle.lock().await;
		if abort_handle.is_some() {
			return;
		}

		let (tx, rx) = oneshot::channel();

		let invalidate_tx = self.invalidate_tx.clone();

		let spawn_fn = actor.spawn_fn.clone();
		let stop_actor = StopActor {
			rx: actor.stop_rx.clone(),
		};

		let task = tokio::spawn(async move {
			(spawn_fn)(stop_actor).await;

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

	#[instrument(skip(self))]
	pub async fn stop(self: &Arc<Self>, name: &str) {
		let actor = {
			let mut actors = self.actors.write().await;

			let Some((actor, run_state)) = actors.get_mut(name) else {
				return;
			};

			if matches!(run_state, ActorRunState::Stopped) {
				warn!("Actor already stopped!");
				return;
			}

			if actor.stop_tx.send(()).await.is_err() {
				error!("Failed to send stop signal to actor");
			}

			*run_state = ActorRunState::Stopped;

			Arc::clone(actor)
		};

		let mut abort_handle = actor.abort_handle.lock().await;

		if let Some(abort_handle) = abort_handle.take() {
			abort_handle.abort();
		}
	}

	pub async fn get_state(&self) -> HashMap<String, bool> {
		let actors = self.actors.read().await;

		let mut state = HashMap::with_capacity(actors.len());

		for (name, (actor, _)) in actors.iter() {
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

pub struct StopActor {
	rx: chan::Receiver<()>,
}

pin_project_lite::pin_project! {
	pub struct StopActorFuture<'recv> {
		#[pin]
		fut: chan::Recv<'recv, ()>,
	}
}

impl Future for StopActorFuture<'_> {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();

		match this.fut.poll(cx) {
			Poll::Ready(res) => {
				if res.is_err() {
					warn!("StopActor channel closed, will stop actor");
				}
				Poll::Ready(())
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

impl<'recv> IntoFuture for &'recv StopActor {
	type Output = ();
	type IntoFuture = StopActorFuture<'recv>;

	fn into_future(self) -> Self::IntoFuture {
		StopActorFuture {
			fut: self.rx.recv(),
		}
	}
}
