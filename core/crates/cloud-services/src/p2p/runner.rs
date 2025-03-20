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
	libraries::{self},
	sync::groups,
};
use sd_crypto::{CryptoRng, SeedableRng};
use sd_prisma::prisma::file_path::cas_id;

use std::{
	collections::HashMap,
	path::PathBuf,
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
use iroh::{key::PublicKey, Endpoint, NodeId};
use quic_rpc::{
	server::{Accepting, RpcChannel, RpcServerError},
	transport::quinn::{QuinnConnector, QuinnListener},
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
	JoinedLibraryCreateArgs, NotifyUser, RecivedGetThumbnailArgs, Ticket, UserResponse,
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
	GetThumbnail {
		device_pub_id: devices::PubId,
		cas_id: cas_id::Type,
		library_pub_id: libraries::PubId,
		tx: oneshot::Sender<RecivedGetThumbnailArgs>,
	},
}

/// We use internal mutability here, but don't worry because there will always be a single
/// [`Runner`] running at a time, so the lock is never contended
pub struct Runner {
	current_device_pub_id: devices::PubId,
	token_refresher: TokenRefresher,
	cloud_services: sd_cloud_schema::Client<
		QuinnConnector<sd_cloud_schema::Response, sd_cloud_schema::Request>,
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
	data_directory: PathBuf,
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
			data_directory: self.data_directory.clone(),
		}
	}
}

struct PendingSyncGroupJoin {
	channel: RpcChannel<Service, QuinnListener<cloud_p2p::Request, cloud_p2p::Response>>,
	request: authorize_new_device_in_sync_group::Request,
	this_device: Device,
	since: Instant,
}

type P2PServerEndpoint = QuinnListener<cloud_p2p::Request, cloud_p2p::Response>;

impl Runner {
	pub async fn new(
		current_device_pub_id: devices::PubId,
		cloud_services: &CloudServices,
		msgs_tx: flume::Sender<Message>,
		endpoint: Endpoint,
		data_directory: PathBuf,
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
			data_directory,
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

				StreamMessage::Message(Message::Request(Request::GetThumbnail {
					device_pub_id,
					cas_id,
					library_pub_id,
					tx,
				})) => self.dispatch_get_thumbnail(device_pub_id, cas_id, library_pub_id, tx),

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

	#[allow(clippy::too_many_lines)]
	fn dispatch_get_thumbnail(
		&self,
		device_pub_id: devices::PubId,
		cas_id: cas_id::Type,
		library_pub_id: libraries::PubId,
		tx: oneshot::Sender<RecivedGetThumbnailArgs>,
	) {
		debug!(?device_pub_id, ?cas_id, "Received request for thumbnail");
		let current_device_pub_id = self.current_device_pub_id;
		let cas_id_clone = cas_id.clone();

		// Put tx in an Arc to allow multiple references to it
		let tx = Arc::new(Mutex::new(Some(tx)));

		let device_connection = self
			.cached_devices_per_group
			.values()
			.find(|(_, devices)| devices.iter().any(|(pub_id, _)| pub_id == &device_pub_id))
			.and_then(|(_, devices)| devices.iter().find(|(pub_id, _)| pub_id == &device_pub_id))
			.ok_or_else(|| {
				error!("Failed to find device in the cached devices list");

				// Use a clone of the channel to send the error response
				let tx_clone = tx.clone();
				spawn(async move {
					if let Some(tx) = tx_clone.lock().await.take() {
						if tx
							.send(RecivedGetThumbnailArgs {
								cas_id: cas_id_clone.clone(),
								error: Some(Error::DeviceNotFound),
							})
							.is_err()
						{
							error!("Failed to send response to user;");
						}
					}
				});
			})
			.expect("Device must be in the cached devices list");

		let (_, device_connection_id) = device_connection;

		debug!("Device Connection ID: {:?}", device_connection_id);
		let data_dir_clone = self.data_directory.clone();

		// Spawn a separate task to avoid blocking the runner
		spawn({
			let endpoint = self.endpoint.clone();
			let device_connection_id = *device_connection_id;
			let tx = tx.clone();
			let cas_id_clone_clone = cas_id.clone();

			async move {
				// Connect to the device
				let client =
					match connect_to_specific_client(&endpoint, &device_connection_id).await {
						Ok(client) => client,
						Err(e) => {
							error!(?e, "Failed to connect to device");
							// Send the error through the channel
							if let Some(tx) = tx.lock().await.take() {
								if tx
									.send(RecivedGetThumbnailArgs {
										cas_id: cas_id_clone_clone,
										error: Some(Error::DeviceNotFound),
									})
									.is_err()
								{
									error!("Failed to send response to user;");
								}
							}
							return;
						}
					};

				// Create the request
				let request = cloud_p2p::get_thumbnail::Request {
					cas_id: cas_id_clone_clone.clone().unwrap_or_default(),
					device_pub_id: current_device_pub_id,
					library_pub_id,
				};

				// Send the request
				match client.get_thumbnail(request).await {
					Ok(Ok(cloud_p2p::get_thumbnail::Response { thumbnail })) => {
						debug!(?cas_id, "Successfully received thumbnail");

						// Convert cas_id to a string
						let cas_id_str = cas_id_clone_clone.clone().unwrap_or_default();

						// If we received a thumbnail, try to save it locally
						if let Some(thumbnail_data) = &thumbnail {
							// Try to save the thumbnail, but don't fail if saving fails
							if let Err(e) = save_remote_thumbnail(
								&cas_id_str,
								thumbnail_data,
								data_dir_clone,
								library_pub_id,
							)
							.await
							{
								error!(?e, "Failed to save remote thumbnail locally, but continuing with response");
							}
						}

						// Send the response via the oneshot channel
						if let Some(tx) = tx.lock().await.take() {
							if tx
								.send(RecivedGetThumbnailArgs {
									cas_id: cas_id_clone_clone.clone(),
									error: None,
								})
								.is_err()
							{
								error!("Failed to send thumbnail response to user");
							}
						}
					}
					Ok(Err(e)) => {
						error!(?e, "Remote device returned error for thumbnail request");
						// Send the error through the channel
						if let Some(tx) = tx.lock().await.take() {
							if tx
								.send(RecivedGetThumbnailArgs {
									cas_id: cas_id_clone_clone.clone(),
									error: Some(Error::RemoteDeviceError),
								})
								.is_err()
							{
								error!("Failed to send response to user;");
							}
						}
					}
					Err(e) => {
						error!(?e, "Failed to send thumbnail request to remote device");
						// Send the error through the channel
						if let Some(tx) = tx.lock().await.take() {
							if tx
								.send(RecivedGetThumbnailArgs {
									cas_id: cas_id_clone_clone.clone(),
									error: Some(Error::InternalError),
								})
								.is_err()
							{
								error!("Failed to send response to user;");
							}
						}
					}
				}
			}
		});
	}

	#[allow(clippy::too_many_lines)]
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

			cloud_p2p::Request::GetThumbnail(req) => {
				if let Err(e) = channel
					.rpc(
						req,
						(),
						|(),
						 cloud_p2p::get_thumbnail::Request {
						     cas_id,
						     device_pub_id,
						     library_pub_id,
						 }| async move {
							debug!(
								?cas_id,
								"Received thumbnail request from device {:?}", device_pub_id
							);

							match fetch_local_thumbnail(
								Some(cas_id.clone()),
								self.data_directory.clone(),
								library_pub_id,
							)
							.await
							{
								Ok(Some(thumbnail_data)) => {
									debug!(?cas_id, "Found thumbnail locally");
									Ok(cloud_p2p::get_thumbnail::Response {
										thumbnail: Some(thumbnail_data),
									})
								}
								Ok(None) => {
									debug!(?cas_id, "Thumbnail not found locally");
									Err(CloudP2PError::Rejected)
								}
								Err(e) => {
									error!(?e, ?cas_id, "Error fetching thumbnail");
									Err(CloudP2PError::Rejected)
								}
							}
						},
					)
					.await
				{
					error!(?e, "Failed to send get thumbnail response;");
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
) -> Result<Client<QuinnConnector<cloud_p2p::Response, cloud_p2p::Request>>, CloudP2PError> {
	for (device_pub_id, device_connection_id) in devices_in_group {
		if let Ok(connection) = endpoint
			.connect(*device_connection_id, CloudP2PALPN::LATEST)
			.await
			.map_err(
				|e| error!(?e, %device_pub_id, "Failed to connect to authorizor device candidate"),
			) {
			debug!(%device_pub_id, "Connected to authorizor device candidate");

			return Ok(Client::new(RpcClient::new(
				QuinnConnector::from_connection(connection),
			)));
		}
	}

	Err(CloudP2PError::UnableToConnect)
}

async fn connect_to_specific_client(
	endpoint: &Endpoint,
	device_connection_id: &PublicKey,
) -> Result<Client<QuinnConnector<cloud_p2p::Response, cloud_p2p::Request>>, CloudP2PError> {
	// Get the connection id by fetching using the device pub id
	let connection = endpoint
		.connect(*device_connection_id, CloudP2PALPN::LATEST)
		.await
		.map_err(|e| {
			error!(?e, "Failed to connect to authorizor device candidate");
			CloudP2PError::UnableToConnect
		})?;
	debug!(%device_connection_id, "Connected to authorizor device candidate");
	Ok(Client::new(RpcClient::new(
		QuinnConnector::from_connection(connection),
	)))
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
		RpcServer::new(QuinnListener::handle_connections(
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

async fn fetch_local_thumbnail(
	cas_id: cas_id::Type,
	data_directory: PathBuf,
	library_pub_id: libraries::PubId,
) -> Result<Option<Vec<u8>>, Error> {
	use tokio::fs;
	use tracing::{debug, error};

	debug!(?cas_id, "Fetching thumbnail from local storage");

	// Convert cas_id to a string
	let cas_id = cas_id.unwrap_or_default();

	let cas_id = sd_core_prisma_helpers::CasId::from(cas_id);

	let thumbnails_directory =
		sd_core_heavy_lifting::media_processor::get_thumbnails_directory(data_directory);

	// Get the shard hex for the cas_id
	let shard_hex = sd_core_heavy_lifting::media_processor::get_shard_hex(&cas_id);

	// First try to find the thumbnail in the specific library folder
	let library_path = thumbnails_directory.join(library_pub_id.to_string());
	let shard_path = library_path.join(shard_hex);
	let thumbnail_path = shard_path.join(format!("{}.webp", cas_id.as_str()));

	debug!("Checking for thumbnail at {:?}", thumbnail_path);

	// If the thumbnail exists in the specific library folder, read it
	if fs::metadata(&thumbnail_path).await.is_ok() {
		match fs::read(&thumbnail_path).await {
			Ok(data) => {
				debug!("Found thumbnail at {:?}", thumbnail_path);
				return Ok(Some(data));
			}
			Err(e) => {
				error!(?e, "Failed to read thumbnail file");
				return Err(Error::InternalError);
			}
		}
	}

	// If not found in the specific library, try the ephemeral directory
	let ephemeral_dir = thumbnails_directory.join("ephemeral");
	let ephemeral_shard_path = ephemeral_dir.join(shard_hex);
	let ephemeral_thumbnail_path = ephemeral_shard_path.join(format!("{}.webp", cas_id.as_str()));

	debug!(
		"Checking for thumbnail in ephemeral at {:?}",
		ephemeral_thumbnail_path
	);

	// If the thumbnail exists in ephemeral, read it
	if fs::metadata(&ephemeral_thumbnail_path).await.is_ok() {
		match fs::read(&ephemeral_thumbnail_path).await {
			Ok(data) => {
				debug!("Found thumbnail at {:?}", ephemeral_thumbnail_path);
				return Ok(Some(data));
			}
			Err(e) => {
				error!(?e, "Failed to read thumbnail file");
				return Err(Error::InternalError);
			}
		}
	}

	// If we still don't have the thumbnail, search all library folders as a fallback
	// This is to handle cases where the library ID might have changed
	let Ok(mut directories) = fs::read_dir(&thumbnails_directory).await else {
		debug!("No thumbnails directory found");
		return Ok(None);
	};

	// Try to find the thumbnail in any other library directories
	while let Ok(Some(entry)) = directories.next_entry().await {
		let dir_path = entry.path();

		// Skip files and already checked directories
		if !dir_path.is_dir() || dir_path == library_path || dir_path == ephemeral_dir {
			continue;
		}

		// Check if thumbnail exists in this directory
		let other_shard_path = dir_path.join(shard_hex);
		let other_thumbnail_path = other_shard_path.join(format!("{}.webp", cas_id.as_str()));

		debug!("Checking for thumbnail at {:?}", other_thumbnail_path);

		if fs::metadata(&other_thumbnail_path).await.is_ok() {
			match fs::read(&other_thumbnail_path).await {
				Ok(data) => {
					debug!("Found thumbnail at {:?}", other_thumbnail_path);
					return Ok(Some(data));
				}
				Err(e) => {
					error!(?e, "Failed to read thumbnail file");
					return Err(Error::InternalError);
				}
			}
		}
	}

	// If we get here, the thumbnail doesn't exist anywhere
	debug!("Thumbnail not found for {}", cas_id.as_str());
	Ok(None)
}

async fn save_remote_thumbnail(
	cas_id: &str,
	thumbnail_data: &[u8],
	data_directory: PathBuf,
	library_pub_id: libraries::PubId,
) -> Result<PathBuf, Error> {
	use tokio::fs;
	use tracing::{debug, error};

	debug!(?cas_id, "Saving remote thumbnail to local storage");

	// Convert to CasId for path computation
	let cas_id = sd_core_prisma_helpers::CasId::from(cas_id);

	// Get the thumbnails directory
	let thumbnails_directory =
		sd_core_heavy_lifting::media_processor::get_thumbnails_directory(data_directory);
	let library_dir = thumbnails_directory.join(library_pub_id.to_string());

	// Get the shard hex for organizing thumbnails
	let shard_hex = sd_core_heavy_lifting::media_processor::get_shard_hex(&cas_id);

	// Create the full directory path
	let shard_dir = library_dir.join(shard_hex);

	// Create the directories if they don't exist
	if let Err(e) = fs::create_dir_all(&shard_dir).await {
		error!(?e, "Failed to create thumbnail directory structure in library folder, falling back to ephemeral");

		// If we can't create in library folder, fall back to ephemeral
		let ephemeral_dir = thumbnails_directory.join("ephemeral");
		let ephemeral_shard_dir = ephemeral_dir.join(shard_hex);

		if let Err(e) = fs::create_dir_all(&ephemeral_shard_dir).await {
			error!(
				?e,
				"Failed to create thumbnail directory structure in ephemeral folder"
			);
			return Err(Error::InternalError);
		}

		// Create the full path for the thumbnail in ephemeral
		let thumbnail_path = ephemeral_shard_dir.join(format!("{}.webp", cas_id.as_str()));

		// Write the thumbnail data to disk
		match fs::write(&thumbnail_path, thumbnail_data).await {
			Ok(()) => {
				debug!(
					"Successfully saved remote thumbnail to ephemeral: {:?}",
					thumbnail_path
				);
				return Ok(thumbnail_path);
			}
			Err(e) => {
				error!(
					?e,
					"Failed to write remote thumbnail to disk in ephemeral folder"
				);
				return Err(Error::InternalError);
			}
		}
	}

	// Create the full path for the thumbnail in the library folder
	let thumbnail_path = shard_dir.join(format!("{}.webp", cas_id.as_str()));

	// Write the thumbnail data to disk
	match fs::write(&thumbnail_path, thumbnail_data).await {
		Ok(()) => {
			debug!(
				"Successfully saved remote thumbnail to library folder: {:?}",
				thumbnail_path
			);
			Ok(thumbnail_path)
		}
		Err(e) => {
			error!(
				?e,
				"Failed to write remote thumbnail to disk in library folder"
			);

			// If writing to library folder fails, try ephemeral as a fallback
			let ephemeral_dir = thumbnails_directory.join("ephemeral");
			let ephemeral_shard_dir = ephemeral_dir.join(shard_hex);

			if let Err(e) = fs::create_dir_all(&ephemeral_shard_dir).await {
				error!(
					?e,
					"Failed to create thumbnail directory structure in ephemeral folder"
				);
				return Err(Error::InternalError);
			}

			let ephemeral_thumbnail_path =
				ephemeral_shard_dir.join(format!("{}.webp", cas_id.as_str()));

			match fs::write(&ephemeral_thumbnail_path, thumbnail_data).await {
				Ok(()) => {
					debug!(
						"Successfully saved remote thumbnail to ephemeral fallback: {:?}",
						ephemeral_thumbnail_path
					);
					Ok(ephemeral_thumbnail_path)
				}
				Err(e) => {
					error!(
						?e,
						"Failed to write remote thumbnail to disk in ephemeral fallback folder"
					);
					Err(Error::InternalError)
				}
			}
		}
	}
}
