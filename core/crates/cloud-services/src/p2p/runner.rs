use crate::{
	p2p::JoinSyncGroupError, sync::ReceiveAndIngestNotifiers, token_refresher::TokenRefresher,
	CloudServices, Error, KeyManager,
};

use sd_cloud_schema::{
	cloud_p2p::{
		self, authorize_new_device_in_sync_group, notify_new_sync_messages, Client, CloudP2PALPN,
		CloudP2PError, Service,
	},
	devices::{self, Device},
	sync::groups,
};
use sd_crypto::{CryptoRng, SeedableRng};

use std::{
	collections::HashMap,
	pin::pin,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
	time::Duration,
};

use dashmap::DashMap;
use flume::SendError;
use futures::StreamExt;
use futures_concurrency::stream::Merge;
use iroh_net::{Endpoint, NodeId};
use quic_rpc::{
	server::{Accepting, RpcChannel, RpcServerError},
	transport::quinn::{QuinnConnection, QuinnServerEndpoint},
	RpcClient, RpcServer,
};
use tokio::{
	spawn,
	sync::{oneshot, Mutex},
	task::JoinHandle,
	time::{interval, Instant, MissedTickBehavior},
};
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, warn};

use super::{
	new_sync_messages_notifier::dispatch_notifier, BasicLibraryCreationArgs, JoinSyncGroupResponse,
	JoinedLibraryCreateArgs, NotifyUser, Ticket, UserResponse,
};

const TEN_SECONDS: Duration = Duration::from_secs(10);
const FIVE_MINUTES: Duration = Duration::from_secs(60 * 5);

#[allow(clippy::large_enum_variant)] // Ignoring because the enum Stop variant will only happen a single time ever
pub enum Message {
	Request(Request),
	RegisterSyncMessageNotifier((groups::PubId, Arc<ReceiveAndIngestNotifiers>)),
	NotifyPeersSyncMessages(groups::PubId),
	UpdateCachedDevices((groups::PubId, Vec<(devices::PubId, NodeId)>)),
	Stop,
}

pub enum Request {
	JoinSyncGroup {
		req: authorize_new_device_in_sync_group::Request,
		devices_in_group: Vec<(devices::PubId, NodeId)>,
		tx: oneshot::Sender<JoinedLibraryCreateArgs>,
	},
}

/// We use internal mutability here, but don't worry because there will always be a single
/// [`Runner`] running at a time, so the lock is never contended
pub struct Runner {
	current_device_pub_id: devices::PubId,
	token_refresher: TokenRefresher,
	cloud_services: sd_cloud_schema::Client<
		QuinnConnection<sd_cloud_schema::Response, sd_cloud_schema::Request>,
	>,
	msgs_tx: flume::Sender<Message>,
	endpoint: Endpoint,
	key_manager: Arc<KeyManager>,
	ticketer: Arc<AtomicU64>,
	notify_user_tx: flume::Sender<NotifyUser>,
	sync_messages_receiver_notifiers_map:
		Arc<DashMap<groups::PubId, Arc<ReceiveAndIngestNotifiers>>>,
	pending_sync_group_join_requests: Arc<Mutex<HashMap<Ticket, PendingSyncGroupJoin>>>,
	cached_devices_per_group: HashMap<groups::PubId, (Instant, Vec<(devices::PubId, NodeId)>)>,
	timeout_checker_buffer: Vec<(Ticket, PendingSyncGroupJoin)>,
}

impl Clone for Runner {
	fn clone(&self) -> Self {
		Self {
			current_device_pub_id: self.current_device_pub_id,
			token_refresher: self.token_refresher.clone(),
			cloud_services: self.cloud_services.clone(),
			msgs_tx: self.msgs_tx.clone(),
			endpoint: self.endpoint.clone(),
			key_manager: Arc::clone(&self.key_manager),
			ticketer: Arc::clone(&self.ticketer),
			notify_user_tx: self.notify_user_tx.clone(),
			sync_messages_receiver_notifiers_map: Arc::clone(
				&self.sync_messages_receiver_notifiers_map,
			),
			pending_sync_group_join_requests: Arc::clone(&self.pending_sync_group_join_requests),
			// Just cache the devices and their node_ids per group
			cached_devices_per_group: HashMap::new(),
			// This one is a temporary buffer only used for timeout checker
			timeout_checker_buffer: vec![],
		}
	}
}

struct PendingSyncGroupJoin {
	channel: RpcChannel<Service, QuinnServerEndpoint<cloud_p2p::Request, cloud_p2p::Response>>,
	request: authorize_new_device_in_sync_group::Request,
	this_device: Device,
	since: Instant,
}

type P2PServerEndpoint = QuinnServerEndpoint<cloud_p2p::Request, cloud_p2p::Response>;

impl Runner {
	pub async fn new(
		current_device_pub_id: devices::PubId,
		cloud_services: &CloudServices,
		msgs_tx: flume::Sender<Message>,
		endpoint: Endpoint,
	) -> Result<Self, Error> {
		Ok(Self {
			current_device_pub_id,
			token_refresher: cloud_services.token_refresher.clone(),
			cloud_services: cloud_services.client().await?,
			msgs_tx,
			endpoint,
			key_manager: cloud_services.key_manager().await?,
			ticketer: Arc::default(),
			notify_user_tx: cloud_services.notify_user_tx.clone(),
			sync_messages_receiver_notifiers_map: Arc::default(),
			pending_sync_group_join_requests: Arc::default(),
			cached_devices_per_group: HashMap::new(),
			timeout_checker_buffer: vec![],
		})
	}

	pub async fn run(
		mut self,
		msgs_rx: flume::Receiver<Message>,
		user_response_rx: flume::Receiver<UserResponse>,
		mut rng: CryptoRng,
	) {
		// Ignoring because this is only used internally and I think that boxing will be more expensive than wasting
		// some extra bytes for smaller variants
		#[allow(clippy::large_enum_variant)]
		enum StreamMessage {
			AcceptResult(
				Result<Accepting<Service, P2PServerEndpoint>, RpcServerError<P2PServerEndpoint>>,
			),
			Message(Message),
			UserResponse(UserResponse),
			Tick,
		}

		let mut ticker = interval(TEN_SECONDS);
		ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

		// FIXME(@fogodev): Update this function to use iroh-net transport instead of quinn
		// when it's implemented
		let (server, server_handle) = setup_server_endpoint(self.endpoint.clone());

		let mut msg_stream = pin!((
			async_stream::stream! {
				loop {
					yield StreamMessage::AcceptResult(server.accept().await);
				}
			},
			msgs_rx.stream().map(StreamMessage::Message),
			user_response_rx.stream().map(StreamMessage::UserResponse),
			IntervalStream::new(ticker).map(|_| StreamMessage::Tick),
		)
			.merge());

		while let Some(msg) = msg_stream.next().await {
			match msg {
				StreamMessage::AcceptResult(Ok(accepting)) => {
					let Ok((request, channel)) = accepting.read_first().await.map_err(|e| {
						error!(?e, "Failed to read first request from a new connection;");
					}) else {
						continue;
					};

					self.handle_request(request, channel).await;
				}

				StreamMessage::AcceptResult(Err(e)) => {
					// TODO(@fogodev): Maybe report this error to the user on a toast?
					error!(?e, "Error accepting connection;");
				}

				StreamMessage::Message(Message::Request(Request::JoinSyncGroup {
					req,
					devices_in_group,
					tx,
				})) => self.dispatch_join_requests(req, devices_in_group, &mut rng, tx),

				StreamMessage::Message(Message::RegisterSyncMessageNotifier((
					group_pub_id,
					notifier,
				))) => {
					self.sync_messages_receiver_notifiers_map
						.insert(group_pub_id, notifier);
				}

				StreamMessage::Message(Message::NotifyPeersSyncMessages(group_pub_id)) => {
					spawn(dispatch_notifier(
						group_pub_id,
						self.current_device_pub_id,
						self.cached_devices_per_group.get(&group_pub_id).cloned(),
						self.msgs_tx.clone(),
						self.cloud_services.clone(),
						self.token_refresher.clone(),
						self.endpoint.clone(),
					));
				}

				StreamMessage::Message(Message::UpdateCachedDevices((
					group_pub_id,
					devices_connections_ids,
				))) => {
					self.cached_devices_per_group
						.insert(group_pub_id, (Instant::now(), devices_connections_ids));
				}

				StreamMessage::UserResponse(UserResponse::AcceptDeviceInSyncGroup {
					ticket,
					accepted,
				}) => {
					self.handle_join_response(ticket, accepted).await;
				}

				StreamMessage::Tick => self.tick().await,

				StreamMessage::Message(Message::Stop) => {
					server_handle.abort();
					break;
				}
			}
		}
	}

	fn dispatch_join_requests(
		&self,
		req: authorize_new_device_in_sync_group::Request,
		devices_in_group: Vec<(devices::PubId, NodeId)>,
		rng: &mut CryptoRng,
		tx: oneshot::Sender<JoinedLibraryCreateArgs>,
	) {
		async fn inner(
			key_manager: Arc<KeyManager>,
			endpoint: Endpoint,
			mut rng: CryptoRng,
			req: authorize_new_device_in_sync_group::Request,
			devices_in_group: Vec<(devices::PubId, NodeId)>,
			tx: oneshot::Sender<JoinedLibraryCreateArgs>,
		) -> Result<JoinSyncGroupResponse, Error> {
			let group_pub_id = req.sync_group.pub_id;
			loop {
				let client =
					match connect_to_first_available_client(&endpoint, &devices_in_group).await {
						Ok(client) => client,
						Err(e) => {
							return Ok(JoinSyncGroupResponse::Failed(e));
						}
					};

				match client
					.authorize_new_device_in_sync_group(req.clone())
					.await?
				{
					Ok(authorize_new_device_in_sync_group::Response {
						authorizor_device,
						keys,
						library_pub_id,
						library_name,
						library_description,
					}) => {
						debug!(
							device_pub_id = %authorizor_device.pub_id,
							%group_pub_id,
							keys_count = keys.len(),
							%library_pub_id,
							library_name,
							"Received join sync group response"
						);

						key_manager
							.add_many_keys(
								group_pub_id,
								keys.into_iter().map(|key| {
									key.as_slice()
										.try_into()
										.expect("critical error, backend has invalid secret keys")
								}),
								&mut rng,
							)
							.await?;

						if tx
							.send(JoinedLibraryCreateArgs {
								pub_id: library_pub_id,
								name: library_name,
								description: library_description,
							})
							.is_err()
						{
							error!("Failed to handle library creation locally from received library data");
							return Ok(JoinSyncGroupResponse::CriticalError);
						}

						return Ok(JoinSyncGroupResponse::Accepted { authorizor_device });
					}

					// In case of timeout, we will try again
					Err(CloudP2PError::TimedOut) => continue,

					Err(e) => return Ok(JoinSyncGroupResponse::Failed(e)),
				}
			}
		}

		spawn({
			let endpoint = self.endpoint.clone();
			let notify_user_tx = self.notify_user_tx.clone();
			let key_manager = Arc::clone(&self.key_manager);
			let rng = CryptoRng::from_seed(rng.generate_fixed());
			async move {
				let sync_group = req.sync_group.clone();

				if let Err(SendError(response)) = notify_user_tx
					.send_async(NotifyUser::ReceivedJoinSyncGroupResponse {
						response: inner(key_manager, endpoint, rng, req, devices_in_group, tx)
							.await
							.unwrap_or_else(|e| {
								error!(
									?e,
									"Failed to issue authorize new device in sync group request;"
								);
								JoinSyncGroupResponse::CriticalError
							}),
						sync_group,
					})
					.await
				{
					error!(?response, "Failed to send response to user;");
				}
			}
		});
	}

	async fn handle_request(
		&self,
		request: cloud_p2p::Request,
		channel: RpcChannel<Service, P2PServerEndpoint>,
	) {
		match request {
			cloud_p2p::Request::AuthorizeNewDeviceInSyncGroup(
				authorize_new_device_in_sync_group::Request {
					sync_group,
					asking_device,
				},
			) => {
				let ticket = Ticket(self.ticketer.fetch_add(1, Ordering::Relaxed));
				let this_device = sync_group
					.devices
					.iter()
					.find(|device| device.pub_id == self.current_device_pub_id)
					.expect(
						"current device must be in the sync group, otherwise we wouldn't be here",
					)
					.clone();

				self.notify_user_tx
					.send_async(NotifyUser::ReceivedJoinSyncGroupRequest {
						ticket,
						asking_device: asking_device.clone(),
						sync_group: sync_group.clone(),
					})
					.await
					.expect("notify_user_tx must never closes!");

				self.pending_sync_group_join_requests.lock().await.insert(
					ticket,
					PendingSyncGroupJoin {
						channel,
						request: authorize_new_device_in_sync_group::Request {
							sync_group,
							asking_device,
						},
						this_device,
						since: Instant::now(),
					},
				);
			}

			cloud_p2p::Request::NotifyNewSyncMessages(req) => {
				if let Err(e) = channel
					.rpc(
						req,
						(),
						|(),
						 notify_new_sync_messages::Request {
						     sync_group_pub_id,
						     device_pub_id,
						 }| async move {
							debug!(%sync_group_pub_id, %device_pub_id, "Received new sync messages notification");
							if let Some(notifier) = self
								.sync_messages_receiver_notifiers_map
								.get(&sync_group_pub_id)
							{
								notifier.notify_receiver();
							} else {
								warn!("Received new sync messages notification for unknown sync group");
							}

							Ok(notify_new_sync_messages::Response)
						},
					)
					.await
				{
					error!(
						?e,
						"Failed to reply to new sync messages notification request"
					);
				}
			}
		}
	}

	async fn handle_join_response(
		&self,
		ticket: Ticket,
		accepted: Option<BasicLibraryCreationArgs>,
	) {
		let Some(PendingSyncGroupJoin {
			channel,
			request,
			this_device,
			..
		}) = self
			.pending_sync_group_join_requests
			.lock()
			.await
			.remove(&ticket)
		else {
			warn!("Received join response for unknown ticket; We probably timed out this request already");
			return;
		};

		let sync_group = request.sync_group.clone();
		let asking_device_pub_id = request.asking_device.pub_id;

		let was_accepted = accepted.is_some();

		let response = if let Some(BasicLibraryCreationArgs {
			id: library_pub_id,
			name: library_name,
			description: library_description,
		}) = accepted
		{
			Ok(authorize_new_device_in_sync_group::Response {
				authorizor_device: this_device,
				keys: self
					.key_manager
					.get_group_keys(request.sync_group.pub_id)
					.await
					.into_iter()
					.map(Into::into)
					.collect(),
				library_pub_id,
				library_name,
				library_description,
			})
		} else {
			Err(CloudP2PError::Rejected)
		};

		if let Err(e) = channel
			.rpc(request, (), |(), _req| async move { response })
			.await
		{
			error!(?e, "Failed to send response to user;");
			self.notify_join_error(sync_group, JoinSyncGroupError::Communication)
				.await;

			return;
		}

		if was_accepted {
			let Ok(access_token) = self
				.token_refresher
				.get_access_token()
				.await
				.map_err(|e| error!(?e, "Failed to get access token;"))
			else {
				self.notify_join_error(sync_group, JoinSyncGroupError::Auth)
					.await;
				return;
			};

			match self
				.cloud_services
				.sync()
				.groups()
				.reply_join_request(groups::reply_join_request::Request {
					access_token,
					group_pub_id: sync_group.pub_id,
					authorized_device_pub_id: asking_device_pub_id,
					authorizor_device_pub_id: self.current_device_pub_id,
				})
				.await
			{
				Ok(Ok(groups::reply_join_request::Response)) => {
					// Everything is Awesome!
				}
				Ok(Err(e)) => {
					error!(?e, "Failed to reply to join request");
					self.notify_join_error(sync_group, JoinSyncGroupError::InternalServer)
						.await;
				}
				Err(e) => {
					error!(?e, "Failed to send reply to join request");
					self.notify_join_error(sync_group, JoinSyncGroupError::Communication)
						.await;
				}
			}
		}
	}

	async fn notify_join_error(
		&self,
		sync_group: groups::GroupWithDevices,
		error: JoinSyncGroupError,
	) {
		self.notify_user_tx
			.send_async(NotifyUser::SendingJoinSyncGroupResponseError { error, sync_group })
			.await
			.expect("notify_user_tx must never closes!");
	}

	async fn tick(&mut self) {
		self.timeout_checker_buffer.clear();

		let mut pending_sync_group_join_requests =
			self.pending_sync_group_join_requests.lock().await;

		for (ticket, pending_sync_group_join) in pending_sync_group_join_requests.drain() {
			if pending_sync_group_join.since.elapsed() > FIVE_MINUTES {
				let PendingSyncGroupJoin {
					channel, request, ..
				} = pending_sync_group_join;

				let asking_device = request.asking_device.clone();

				let notify_message = match channel
					.rpc(request, (), |(), _req| async move {
						Err(CloudP2PError::TimedOut)
					})
					.await
				{
					Ok(()) => NotifyUser::TimedOutJoinRequest {
						device: asking_device,
						succeeded: true,
					},
					Err(e) => {
						error!(?e, "Failed to send timed out response to user;");
						NotifyUser::TimedOutJoinRequest {
							device: asking_device,
							succeeded: false,
						}
					}
				};

				self.notify_user_tx
					.send_async(notify_message)
					.await
					.expect("notify_user_tx must never closes!");
			} else {
				self.timeout_checker_buffer
					.push((ticket, pending_sync_group_join));
			}
		}

		pending_sync_group_join_requests.extend(self.timeout_checker_buffer.drain(..));
	}
}

async fn connect_to_first_available_client(
	endpoint: &Endpoint,
	devices_in_group: &[(devices::PubId, NodeId)],
) -> Result<Client<QuinnConnection<cloud_p2p::Response, cloud_p2p::Request>>, CloudP2PError> {
	for (device_pub_id, device_connection_id) in devices_in_group {
		if let Ok(connection) = endpoint
			.connect(*device_connection_id, CloudP2PALPN::LATEST)
			.await
			.map_err(
				|e| error!(?e, %device_pub_id, "Failed to connect to authorizor device candidate"),
			) {
			debug!(%device_pub_id, "Connected to authorizor device candidate");

			return Ok(Client::new(RpcClient::new(
				QuinnConnection::from_connection(connection),
			)));
		}
	}

	Err(CloudP2PError::UnableToConnect)
}

fn setup_server_endpoint(
	endpoint: Endpoint,
) -> (RpcServer<Service, P2PServerEndpoint>, JoinHandle<()>) {
	let local_addr = {
		let (ipv4_addr, maybe_ipv6_addr) = endpoint.bound_sockets();
		// Trying to give preference to IPv6 addresses because it's 2024
		maybe_ipv6_addr.unwrap_or(ipv4_addr)
	};

	let (connections_tx, connections_rx) = flume::bounded(16);

	(
		RpcServer::new(QuinnServerEndpoint::handle_connections(
			connections_rx,
			local_addr,
		)),
		spawn(async move {
			while let Some(connecting) = endpoint.accept().await {
				if let Ok(connection) = connecting.await.map_err(|e| {
					warn!(?e, "Cloud P2P failed to accept connection");
				}) {
					if connections_tx.send_async(connection).await.is_err() {
						warn!("Connection receiver dropped");
						break;
					}
				}
			}
		}),
	)
}
