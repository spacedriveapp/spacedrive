use std::{
	future::Future,
	net::SocketAddr,
	pin::Pin,
	task::{Context, Poll},
};

use tokio::sync::broadcast;

use crate::{Manager, ManagerState, PeerId};

/// can be registered into the P2PManager to extend it's functionality
pub trait Component: Send + 'static {
	/// TODO
	// type InEvent;

	/// TODO
	type OutEvent: Clone;

	/// Advertise service to the network.
	/// This will be called when a change to the listen_addr's is made.
	fn advertise(self: Pin<&mut Self>) {}

	/// Get possible connection candidates for this service.
	/// This is used when the node is trying to connect to a peer.
	fn get_candidates(self: Pin<&mut Self>, peer_id: PeerId, candidates: &mut Vec<SocketAddr>) {}

	/// Handle an internal event.
	fn on_event(self: Pin<&mut Self>, event: InternalEvent) {}

	/// TODO
	fn poll(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		state: &mut ManagerState,
	) -> Poll<Option<Self::OutEvent>> {
		Poll::Ready(None)
	}
}

pub trait ErasedComponent {
	fn advertise(self: Pin<&mut Self>);

	fn get_candidates(self: Pin<&mut Self>, peer_id: PeerId, candidates: &mut Vec<SocketAddr>);

	fn on_event(self: Pin<&mut Self>, event: InternalEvent);

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>, state: &mut ManagerState) -> Poll<()>;
}

impl<C: Component> ErasedComponent for C {
	fn advertise(self: Pin<&mut Self>) {
		C::advertise(self)
	}

	fn get_candidates(self: Pin<&mut Self>, peer_id: PeerId, candidates: &mut Vec<SocketAddr>) {
		C::get_candidates(self, peer_id, candidates)
	}

	fn on_event(self: Pin<&mut Self>, event: InternalEvent) {
		C::on_event(self, event)
	}

	// TODO: Custom enum instead of `Poll`?
	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>, state: &mut ManagerState) -> Poll<()> {
		match C::poll(self, cx, state) {
			Poll::Ready(e) => {
				// TODO

				Poll::Ready(())
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

/// Wrapper for addressing multiple services at once
#[derive(Default)]
pub(crate) struct Components(Vec<Pin<Box<dyn ErasedComponent>>>);

impl Components {
	pub fn push(&mut self, service: Pin<Box<dyn ErasedComponent>>) {
		// 32 comes from `streamunordered` crate which itself comes from `futures` I think.
		// This is to avoid a complicated `Future` implementation and really more than 32 is an edge case we don't care about, rn.
		debug_assert!(
			self.0.len() < 32,
			"Many services will starve the async runtime!"
		);

		self.0.push(service);
	}

	pub fn emit(&mut self, event: InternalEvent) {
		for service in self.0.iter_mut() {
			service.as_mut().on_event(event.clone());
		}
	}

	pub fn get_candidates(&mut self, peer_id: PeerId, candidates: &mut Vec<SocketAddr>) {
		for service in self.0.iter_mut() {
			service.as_mut().get_candidates(peer_id, candidates);
		}
	}

	pub fn advertise(&mut self) {
		for service in self.0.iter_mut() {
			service.as_mut().advertise();
		}
	}

	// Can be turned into a `Future` using `poll_fn`
	pub fn poll(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		state: &mut ManagerState,
	) -> Poll<()> {
		let mut pending = false;

		// TODO: Removing complete futures from poll rotation

		// We poll all futures on each `.poll` to ensure a component isn't starved
		for fut in self.0.iter_mut() {
			match fut.as_mut().poll(cx, state) {
				Poll::Ready(()) => {}
				Poll::Pending => pending = true,
			}
		}

		// Yield back to runtime to avoid straving other tasks
		// But schedule us to run again cause otherwise we will stall
		if !pending {
			cx.waker().wake_by_ref();
		}

		Poll::Pending
	}
}

pub struct ComponentRef<T: Clone>(broadcast::Sender<T>);

impl<T: Clone> ComponentRef<T> {}

// TODO: Maybe merging with `ManagerStreamAction`
/// Internal event for P2PManager to emit to services
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum InternalEvent {
	NewListenAddr(SocketAddr),
	ExpiredListenAddr(SocketAddr),
	Shutdown,
}
