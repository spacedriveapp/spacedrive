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
	future::{Future, IntoFuture},
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
	sync::{broadcast, RwLock},
	task::JoinHandle,
	time::timeout,
};
use tracing::{error, instrument, warn};

const ONE_MINUTE: Duration = Duration::from_secs(60);

type ActorFn = dyn Fn(Stopper) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync;

pub struct Actor {
	spawn_fn: Arc<ActorFn>,
	maybe_handle: Option<JoinHandle<()>>,
	is_running: Arc<AtomicBool>,
	stop_tx: chan::Sender<()>,
	stop_rx: chan::Receiver<()>,
}

pub struct Actors {
	pub invalidate_rx: broadcast::Receiver<()>,
	invalidate_tx: broadcast::Sender<()>,
	actors: Arc<RwLock<HashMap<&'static str, Actor>>>,
}

impl Actors {
	pub async fn declare<Fut>(
		self: &Arc<Self>,
		name: &'static str,
		actor_fn: impl FnOnce(Stopper) -> Fut + Send + Sync + Clone + 'static,
		autostart: bool,
	) where
		Fut: Future<Output = ()> + Send + 'static,
	{
		let (stop_tx, stop_rx) = chan::bounded(1);

		self.actors.write().await.insert(
			name,
			Actor {
				spawn_fn: Arc::new(move |stop| Box::pin((actor_fn.clone())(stop))),
				maybe_handle: None,
				is_running: Arc::new(AtomicBool::new(false)),
				stop_tx,
				stop_rx,
			},
		);

		if autostart {
			self.start(name).await;
		}
	}

	#[instrument(skip(self))]
	pub async fn start(self: &Arc<Self>, name: &str) {
		if let Some(actor) = self.actors.write().await.get_mut(name) {
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
				let spawn_fn = Arc::clone(&actor.spawn_fn);

				let stop_actor = Stopper(actor.stop_rx.clone());

				async move {
					if (AssertUnwindSafe((spawn_fn)(stop_actor)))
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
	pub async fn stop(self: &Arc<Self>, name: &str) {
		if let Some(actor) = self.actors.write().await.get_mut(name) {
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

	pub async fn get_state(&self) -> HashMap<String, bool> {
		self.actors
			.read()
			.await
			.iter()
			.map(|(&name, actor)| (name.to_string(), actor.is_running.load(Ordering::Relaxed)))
			.collect()
	}
}

impl Default for Actors {
	fn default() -> Self {
		let (invalidate_tx, invalidate_rx) = broadcast::channel(1);

		Self {
			actors: Arc::default(),
			invalidate_rx,
			invalidate_tx,
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
