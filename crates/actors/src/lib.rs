#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use std::{
	collections::HashMap,
	fmt,
	future::{Future, IntoFuture},
	hash::Hash,
	marker::PhantomData,
	panic::{panic_any, AssertUnwindSafe},
	pin::Pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	task::{Context, Poll},
	time::Duration,
};

use async_channel as chan;
use futures::FutureExt;
use tokio::{
	spawn,
	sync::{broadcast, Mutex, RwLock},
	task::JoinHandle,
	time::timeout,
};
use tracing::{error, instrument, warn};

const ONE_MINUTE: Duration = Duration::from_secs(60);

pub trait ActorId: Hash + Eq + Send + Sync + Copy + fmt::Debug + fmt::Display + 'static {}

impl<T: Hash + Eq + Send + Sync + Copy + fmt::Debug + fmt::Display + 'static> ActorId for T {}

pub trait Actor<Id: ActorId>: Send + Sync + 'static {
	const IDENTIFIER: Id;

	fn run(&mut self, stop: Stopper) -> impl Future<Output = ()> + Send;
}

mod sealed {
	pub trait Sealed {}
}

#[async_trait::async_trait]
pub trait DynActor<Id: ActorId>: Send + Sync + sealed::Sealed + 'static {
	async fn run(&mut self, stop: Stopper);
}

pub trait IntoActor<Id: ActorId>: Send + Sync {
	fn into_actor(self) -> (Id, Box<dyn DynActor<Id>>);
}

struct AnyActor<Id: ActorId, A: Actor<Id>> {
	actor: A,
	_marker: PhantomData<Id>,
}

impl<Id: ActorId, A: Actor<Id>> sealed::Sealed for AnyActor<Id, A> {}

#[async_trait::async_trait]
impl<Id: ActorId, A: Actor<Id>> DynActor<Id> for AnyActor<Id, A> {
	async fn run(&mut self, stop: Stopper) {
		self.actor.run(stop).await;
	}
}

impl<Id: ActorId, A: Actor<Id>> IntoActor<Id> for A {
	fn into_actor(self) -> (Id, Box<dyn DynActor<Id>>) {
		(
			A::IDENTIFIER,
			Box::new(AnyActor {
				actor: self,
				_marker: PhantomData,
			}),
		)
	}
}

struct ActorHandler<Id: ActorId> {
	actor: Arc<Mutex<Box<dyn DynActor<Id>>>>,
	maybe_handle: Option<JoinHandle<()>>,
	is_running: Arc<AtomicBool>,
	stop_tx: chan::Sender<()>,
	stop_rx: chan::Receiver<()>,
}

/// Actors holder, holds all actors for some generic purpose, like for cloud sync.
/// You should use an enum to identify the actors.
pub struct ActorsCollection<Id: ActorId> {
	pub invalidate_rx: broadcast::Receiver<()>,
	invalidate_tx: broadcast::Sender<()>,
	actors_map: Arc<RwLock<HashMap<Id, ActorHandler<Id>>>>,
}

impl<Id: ActorId> ActorsCollection<Id> {
	pub async fn declare(&self, actor: impl IntoActor<Id>) {
		async fn inner<Id: ActorId>(
			this: &ActorsCollection<Id>,
			identifier: Id,
			actor: Box<dyn DynActor<Id>>,
		) {
			let (stop_tx, stop_rx) = chan::bounded(1);

			this.actors_map.write().await.insert(
				identifier,
				ActorHandler {
					actor: Arc::new(Mutex::new(actor)),
					maybe_handle: None,
					is_running: Arc::new(AtomicBool::new(false)),
					stop_tx,
					stop_rx,
				},
			);
		}

		let (identifier, actor) = actor.into_actor();
		inner(self, identifier, actor).await;
	}

	pub async fn declare_many_boxed(
		&self,
		actors: impl IntoIterator<Item = (Id, Box<dyn DynActor<Id>>)> + Send,
	) {
		let mut actor_map = self.actors_map.write().await;

		for (id, actor) in actors {
			let (stop_tx, stop_rx) = chan::bounded(1);

			actor_map.insert(
				id,
				ActorHandler {
					actor: Arc::new(Mutex::new(actor)),
					maybe_handle: None,
					is_running: Arc::new(AtomicBool::new(false)),
					stop_tx,
					stop_rx,
				},
			);
		}
	}

	#[instrument(skip(self))]
	pub async fn start(&self, identifier: Id) {
		let mut actors_map = self.actors_map.write().await;
		if let Some(actor) = actors_map.get_mut(&identifier) {
			if actor.is_running.load(Ordering::Acquire) {
				warn!("Actor already running!");
				return;
			}

			let invalidate_tx = self.invalidate_tx.clone();

			let is_running = Arc::clone(&actor.is_running);

			is_running.store(true, Ordering::Release);

			if invalidate_tx.send(()).is_err() {
				warn!("Failed to send invalidate signal");
			}

			if let Some(handle) = actor.maybe_handle.take() {
				if handle.await.is_err() {
					// This should never happen, as we're trying to catch the panic below with
					// `catch_unwind`.
					error!("Actor unexpectedly panicked");
				}
			}

			actor.maybe_handle = Some(spawn({
				let stop_actor = Stopper(actor.stop_rx.clone());
				let actor = Arc::clone(&actor.actor);

				async move {
					if (AssertUnwindSafe(
						actor
							.try_lock()
							.expect("actors can only have a single run at a time")
							.run(stop_actor),
					))
					.catch_unwind()
					.await
					.is_err()
					{
						error!("Actor unexpectedly panicked");
					}

					is_running.store(false, Ordering::Release);

					if invalidate_tx.send(()).is_err() {
						warn!("Failed to send invalidate signal");
					}
				}
			}));
		}
	}

	#[instrument(skip(self))]
	pub async fn stop(&self, identifier: Id) {
		let mut actors_map = self.actors_map.write().await;
		if let Some(actor) = actors_map.get_mut(&identifier) {
			if !actor.is_running.load(Ordering::Acquire) {
				warn!("Actor already stopped!");
				return;
			}

			if actor.stop_tx.send(()).await.is_ok() {
				wait_stop_or_abort(actor.maybe_handle.take()).await;

				assert!(
					!actor.is_running.load(Ordering::Acquire),
					"actor handle finished without setting actor to stopped"
				);
			} else {
				error!("Failed to send stop signal to actor, will check if it's already stopped or abort otherwise");
				wait_stop_or_abort(actor.maybe_handle.take()).await;
			}
		}
	}

	pub async fn get_state(&self) -> Vec<(String, bool)> {
		self.actors_map
			.read()
			.await
			.iter()
			.map(|(identifier, actor)| {
				(
					identifier.to_string(),
					actor.is_running.load(Ordering::Relaxed),
				)
			})
			.collect()
	}
}

impl<Id: ActorId> Default for ActorsCollection<Id> {
	fn default() -> Self {
		let (invalidate_tx, invalidate_rx) = broadcast::channel(1);

		Self {
			actors_map: Arc::default(),
			invalidate_rx,
			invalidate_tx,
		}
	}
}

impl<Id: ActorId> Clone for ActorsCollection<Id> {
	fn clone(&self) -> Self {
		Self {
			actors_map: Arc::clone(&self.actors_map),
			invalidate_rx: self.invalidate_rx.resubscribe(),
			invalidate_tx: self.invalidate_tx.clone(),
		}
	}
}

pub struct Stopper(chan::Receiver<()>);

impl Stopper {
	#[must_use]
	pub fn check_stop(&self) -> bool {
		self.0.try_recv().is_ok()
	}
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

impl<'recv> IntoFuture for &'recv Stopper {
	type Output = ();
	type IntoFuture = StopActorFuture<'recv>;

	fn into_future(self) -> Self::IntoFuture {
		Self::IntoFuture { fut: self.0.recv() }
	}
}

async fn wait_stop_or_abort(maybe_handle: Option<JoinHandle<()>>) {
	if let Some(handle) = maybe_handle {
		let abort_handle = handle.abort_handle();

		match timeout(ONE_MINUTE, handle).await {
			Ok(Ok(())) => { /* Everything is Awesome! */ }
			Ok(Err(e)) => {
				// This should never happen, as we're trying to catch the panic with
				// `catch_unwind`.
				if e.is_panic() {
					let p = e.into_panic();
					error!("Actor unexpectedly panicked, we will pop up the panic!");
					panic_any(p);
				}
			}
			Err(_) => {
				error!("Actor failed to gracefully stop in the allotted time, will force abortion");
				abort_handle.abort();
			}
		}
	}
}
