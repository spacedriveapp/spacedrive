use sd_core_cloud_services::CloudServices;

use sd_cloud_schema::devices;

use std::{pin::pin, sync::Arc, time::Duration};

use anyhow::Context as _;
use futures::StreamExt as _;
use futures_concurrency::stream::Merge as _;
use iroh::NodeId;
use quic_rpc::{
	server::{Accepting, RpcServerError},
	Listener, RpcServer,
};
use tokio::{
	spawn,
	sync::{oneshot, RwLock},
	task::JoinError,
	time::timeout,
};
use tracing::{error, info, warn};

use super::schema;

mod router;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct Server {
	current_device_pub_id: devices::PubId,
	cloud_services: Arc<RwLock<Option<CloudServices>>>,
	known_devices: Arc<RwLock<Vec<NodeId>>>,
}

impl Server {
	pub fn new(
		current_device_pub_id: devices::PubId,
		cloud_services: Arc<RwLock<Option<CloudServices>>>,
		known_devices: Arc<RwLock<Vec<NodeId>>>,
	) -> Self {
		Self {
			current_device_pub_id,
			cloud_services,
			known_devices,
		}
	}

	async fn handle_single_request(
		self,
		accepting: Accepting<schema::Service, impl Listener<schema::Service>>,
		out_tx: flume::Sender<Result<anyhow::Result<()>, JoinError>>,
	) {
		async fn inner(
			server: Server,
			accepting: Accepting<schema::Service, impl Listener<schema::Service>>,
		) -> anyhow::Result<()> {
			let (req, chan) = accepting
				.read_first()
				.await
				.context("Failed to receive request")?;

			router::handle(server, req, chan).await
		}

		// Running on a detached task to avoid panicking the main task
		let res = spawn(inner(self, accepting)).await;
		out_tx.send_async(res).await.expect("channel never closes");
	}

	pub fn dispatch(
		self,
		rpc_server: RpcServer<schema::Service, impl Listener<schema::Service>>,
		cancel_rx: flume::Receiver<oneshot::Sender<()>>,
	) {
		spawn({
			async move {
				loop {
					info!("Starting P2P Server");
					if let Err(e) =
						spawn(self.clone().run_loop(rpc_server.clone(), cancel_rx.clone())).await
					{
						if e.is_panic() {
							error!(?e, "P2P Server crashed, restarting...");
						} else {
							break;
						}
					}
				}
			}
		});
	}

	async fn run_loop(
		self,
		rpc_server: RpcServer<schema::Service, impl Listener<schema::Service>>,
		cancel_rx: flume::Receiver<oneshot::Sender<()>>,
	) {
		enum StreamMessage<Listener: quic_rpc::Listener<schema::Service>> {
			AcceptResult(Result<Accepting<schema::Service, Listener>, RpcServerError<Listener>>),
			RequestOutcome(Result<anyhow::Result<()>, JoinError>),
			Shutdown(oneshot::Sender<()>),
		}

		let (out_tx, out_rx) = flume::bounded(32);

		let mut msg_stream = pin!((
			async_stream::stream! {
				loop {
					yield StreamMessage::AcceptResult(rpc_server.accept().await);
				}
			},
			cancel_rx.stream().map(StreamMessage::Shutdown),
			out_rx.stream().map(StreamMessage::RequestOutcome)
		)
			.merge());

		let mut inflight_count = 0u32;

		info!("P2P listening for connections...");

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::AcceptResult(Ok(accepting)) => {
					spawn(
						self.clone()
							.handle_single_request(accepting, out_tx.clone()),
					);
					inflight_count += 1;
				}
				StreamMessage::AcceptResult(Err(e)) => {
					error!(?e, "Failed to accept request;");
				}

				StreamMessage::RequestOutcome(out) => {
					process_request_outcome(out);
					inflight_count -= 1;
				}

				StreamMessage::Shutdown(tx) => {
					// Received an Interrupt signal, which means the user wants to stop the server,
					// so we wait for all inflight requests to finish before exiting
					// this way we're doing a graceful shutdown

					let wait_all_to_finish = async {
						while inflight_count > 0 {
							process_request_outcome(
								// SAFETY: channel never closes
								out_rx.recv_async().await.expect("channel never closes"),
							);
							inflight_count -= 1;
						}
					};

					if let Err(elapsed) = timeout(SHUTDOWN_TIMEOUT, wait_all_to_finish).await {
						warn!(?elapsed, %inflight_count, "Server graceful shutdown timed out");
					} else {
						info!("Server graceful shutdown complete!");
					}

					if tx.send(()).is_err() {
						warn!("Failed to send P2P shutdown completion response;");
					}

					break;
				}
			}
		}
	}
}

fn process_request_outcome(out: Result<anyhow::Result<()>, JoinError>) {
	match out {
		Ok(Err(e)) => {
			error!(?e, "Failed to handle request;");
		}
		Err(e) if e.is_panic() => {
			if let Some(msg) = e.into_panic().downcast_ref::<&str>() {
				error!(?msg, "Panic in request handler!");
			} else {
				error!("Some unknown panic in request handler!");
			}
		}
		Ok(Ok(())) | Err(_) => {
			// The request was handled successfully, or the JoinHandle was aborted,
			// which can't happen because we don't even have the handle, so...
			// ...
			// Everything is Awesome!
		}
	}
}
