#![allow(dead_code, unused_variables)] // TODO: Reenable once this is working

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{mpsc, oneshot};

pub enum PeerRequest {
	Guest(guest::PeerRequest),
	Host(host::PeerRequest),
}

enum PlaceholderP2PAction {
	SubmitPeeringPassword {
		peer_id: String,
		password: String,
		tx: oneshot::Sender<bool>,
	},
}

pub mod guest {
	use super::*;

	#[derive(Type, Deserialize)]
	pub enum Action {
		PromptPassword,
		ProcessPassword { password: String },
	}

	#[derive(Type, Serialize, Clone)]
	pub enum State {
		Start,
		AwaitingPassword { prev_invalid: bool },
		AwaitingConfirmation,
		ChallengeSuccess,
	}

	pub struct PeerRequest {
		pub tx: mpsc::Sender<Action>,
		pub peer_id: String,
	}

	struct ActorArgs {
		peer_id: String,
		p2p: mpsc::Sender<PlaceholderP2PAction>,
	}

	async fn loop_until<T, R>(rx: &mut mpsc::Receiver<T>, func: impl Fn(T) -> Option<R>) -> R {
		loop {
			let Some(msg) = rx.recv().await else {
                panic!()
            };

			if let Some(d) = func(msg) {
				break d;
			}
		}
	}

	impl PeerRequest {
		pub fn new_actor(peer_id: String) -> (Self, mpsc::Receiver<State>) {
			let (itx, irx) = mpsc::channel(8);
			let (otx, orx) = mpsc::channel(8);
			let (p2ptx, _) = mpsc::channel(8);

			tokio::spawn(Self::actor(
				otx,
				irx,
				ActorArgs {
					peer_id: peer_id.clone(),
					p2p: p2ptx,
				},
			));

			(Self { tx: itx, peer_id }, orx)
		}

		async fn actor(
			state_tx: mpsc::Sender<State>,
			mut action_rx: mpsc::Receiver<Action>,
			ActorArgs { peer_id, p2p }: ActorArgs,
		) {
			let send_state = |state| async { state_tx.send(state).await.ok() };

			send_state(State::Start).await;

			loop_until(&mut action_rx, |msg| {
				matches!(Action::PromptPassword, msg).then_some(())
			})
			.await;

			send_state(State::AwaitingPassword {
				prev_invalid: false,
			})
			.await;

			loop {
				let password = loop_until(&mut action_rx, |msg| match msg {
					Action::ProcessPassword { password } => Some(password),
					_ => None,
				})
				.await;

				let (tx, rx) = oneshot::channel();
				p2p.send(PlaceholderP2PAction::SubmitPeeringPassword {
					peer_id: peer_id.clone(),
					password,
					tx,
				})
				.await
				.ok();

				if rx.await.unwrap() {
					break;
				}

				send_state(State::AwaitingPassword { prev_invalid: true }).await;
			}

			send_state(State::ChallengeSuccess).await;
		}

		pub async fn submit_password(&self, password: String) {
			self.tx
				.send(Action::ProcessPassword { password })
				.await
				.ok();
		}
	}
}

pub mod host {
	use super::*;

	#[derive(Type, Deserialize)]
	pub enum Action {
		PromptPassword,
		ProcessPassword { password: String },
	}

	#[derive(Type, Serialize, Clone)]
	pub enum State {
		AwaitingResponse,
		ChallengeReceived,
	}

	pub struct PeerRequest {
		pub tx: mpsc::Sender<Action>,
		pub peer_id: String,
	}

	// impl PeerRequest {
	// 	pub fn new_actor() -> (Self, mpsc::Receiver<State>) {}
	// }
}
