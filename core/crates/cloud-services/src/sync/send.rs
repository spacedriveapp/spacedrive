use crate::{CloudServices, Error, KeyManager};

use sd_core_sync::{CompressedCRDTOperationsPerModelPerDevice, SyncEvent, SyncManager, NTP64};

use sd_actors::{Actor, Stopper};
use sd_cloud_schema::{
	devices,
	error::{ClientSideError, NotFoundError},
	sync::{groups, messages},
	Client, Request, Response,
};
use sd_crypto::{
	cloud::{OneShotEncryption, SecretKey, StreamEncryption},
	primitives::EncryptedBlock,
	CryptoRng, SeedableRng,
};
use sd_utils::{datetime_to_timestamp, timestamp_to_datetime};

use std::{
	future::IntoFuture,
	pin::pin,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::{Duration, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use futures::{FutureExt, StreamExt, TryStreamExt};
use futures_concurrency::future::{Race, TryJoin};
use quic_rpc::transport::quinn::QuinnConnection;
use tokio::{
	sync::{broadcast, Notify},
	time::sleep,
};
use tracing::{debug, error};
use uuid::Uuid;

use super::{SyncActors, ONE_MINUTE};

const TEN_SECONDS: Duration = Duration::from_secs(10);

const MESSAGES_COLLECTION_SIZE: u32 = 10_000;

enum RaceNotifiedOrStopped {
	Notified,
	Stopped,
}

enum LoopStatus {
	SentMessages,
	Idle,
}

type LatestTimestamp = NTP64;

#[derive(Debug)]
pub struct Sender {
	sync_group_pub_id: groups::PubId,
	sync: SyncManager,
	cloud_services: Arc<CloudServices>,
	cloud_client: Client<QuinnConnection<Response, Request>>,
	key_manager: Arc<KeyManager>,
	is_active: Arc<AtomicBool>,
	state_notify: Arc<Notify>,
	rng: CryptoRng,
	maybe_latest_timestamp: Option<LatestTimestamp>,
}

impl Actor<SyncActors> for Sender {
	const IDENTIFIER: SyncActors = SyncActors::Sender;

	async fn run(&mut self, stop: Stopper) {
		loop {
			self.is_active.store(true, Ordering::Relaxed);
			self.state_notify.notify_waiters();

			let res = self.run_loop_iteration().await;

			self.is_active.store(false, Ordering::Relaxed);

			match res {
				Ok(LoopStatus::SentMessages) => {
					if let Ok(cloud_p2p) = self.cloud_services.cloud_p2p().await.map_err(|e| {
						error!(?e, "Failed to get cloud p2p client on sender actor");
					}) {
						cloud_p2p
							.notify_new_sync_messages(self.sync_group_pub_id)
							.await;
					}
				}

				Ok(LoopStatus::Idle) => {}

				Err(e) => {
					error!(?e, "Error during cloud sync sender actor iteration");
					sleep(ONE_MINUTE).await;
					continue;
				}
			}

			self.state_notify.notify_waiters();

			if matches!(
				(
					// recreate subscription each time so that existing messages are dropped
					wait_notification(self.sync.subscribe()),
					stop.into_future().map(|()| RaceNotifiedOrStopped::Stopped),
				)
					.race()
					.await,
				RaceNotifiedOrStopped::Stopped
			) {
				break;
			}

			sleep(TEN_SECONDS).await;
		}
	}
}

impl Sender {
	pub async fn new(
		sync_group_pub_id: groups::PubId,
		sync: SyncManager,
		cloud_services: Arc<CloudServices>,
		is_active: Arc<AtomicBool>,
		state_notify: Arc<Notify>,
		rng: CryptoRng,
	) -> Result<Self, Error> {
		let (cloud_client, key_manager) = (cloud_services.client(), cloud_services.key_manager())
			.try_join()
			.await?;

		Ok(Self {
			sync_group_pub_id,
			sync,
			cloud_services,
			cloud_client,
			key_manager,
			is_active,
			state_notify,
			rng,
			maybe_latest_timestamp: None,
		})
	}

	async fn run_loop_iteration(&mut self) -> Result<LoopStatus, Error> {
		debug!("Starting cloud sender actor loop iteration");

		let current_device_pub_id = devices::PubId(Uuid::from(&self.sync.device_pub_id));

		let (key_hash, secret_key) = self
			.key_manager
			.get_latest_key(self.sync_group_pub_id)
			.await
			.ok_or(Error::MissingSyncGroupKey(self.sync_group_pub_id))?;

		let current_latest_timestamp = self.get_latest_timestamp(current_device_pub_id).await?;

		let mut crdt_ops_stream = pin!(self.sync.stream_device_ops(
			&self.sync.device_pub_id,
			MESSAGES_COLLECTION_SIZE,
			current_latest_timestamp
		));

		let mut status = LoopStatus::Idle;

		let mut new_latest_timestamp = current_latest_timestamp;

		debug!(
			chunk_size = MESSAGES_COLLECTION_SIZE,
			"Trying to fetch chunk of sync messages from the database"
		);
		while let Some(ops_res) = crdt_ops_stream.next().await {
			let ops = ops_res?;

			let (Some(first), Some(last)) = (ops.first(), ops.last()) else {
				break;
			};

			debug!("Got first and last sync messages");

			#[allow(clippy::cast_possible_truncation)]
			let operations_count = ops.len() as u32;

			debug!(operations_count, "Got chunk of sync messages");

			new_latest_timestamp = last.timestamp;

			let start_time = timestamp_to_datetime(first.timestamp);
			let end_time = timestamp_to_datetime(last.timestamp);

			// Ignoring this device_pub_id here as we already know it
			let (_device_pub_id, compressed_ops) =
				CompressedCRDTOperationsPerModelPerDevice::new_single_device(ops);

			let messages_bytes = rmp_serde::to_vec_named(&compressed_ops)
				.map_err(Error::SerializationFailureToPushSyncMessages)?;

			let encrypted_messages =
				encrypt_messages(&secret_key, &mut self.rng, messages_bytes).await?;

			let encrypted_messages_size = encrypted_messages.len();

			debug!(
				operations_count,
				encrypted_messages_size, "Sending sync messages to cloud",
			);

			self.cloud_client
				.sync()
				.messages()
				.push(messages::push::Request {
					access_token: self
						.cloud_services
						.token_refresher
						.get_access_token()
						.await?,
					group_pub_id: self.sync_group_pub_id,
					device_pub_id: current_device_pub_id,
					key_hash: key_hash.clone(),
					operations_count,
					time_range: (start_time, end_time),
					encrypted_messages,
				})
				.await??;

			debug!(
				operations_count,
				encrypted_messages_size, "Sent sync messages to cloud",
			);

			status = LoopStatus::SentMessages;
		}

		self.maybe_latest_timestamp = Some(new_latest_timestamp);

		debug!("Finished cloud sender actor loop iteration");

		Ok(status)
	}

	async fn get_latest_timestamp(
		&self,
		current_device_pub_id: devices::PubId,
	) -> Result<LatestTimestamp, Error> {
		if let Some(latest_timestamp) = &self.maybe_latest_timestamp {
			Ok(*latest_timestamp)
		} else {
			let latest_time = match self
				.cloud_client
				.sync()
				.messages()
				.get_latest_time(messages::get_latest_time::Request {
					access_token: self
						.cloud_services
						.token_refresher
						.get_access_token()
						.await?,
					group_pub_id: self.sync_group_pub_id,
					kind: messages::get_latest_time::Kind::ForCurrentDevice(current_device_pub_id),
				})
				.await?
			{
				Ok(messages::get_latest_time::Response {
					latest_time,
					latest_device_pub_id,
				}) => {
					assert_eq!(latest_device_pub_id, current_device_pub_id);
					latest_time
				}

				Err(sd_cloud_schema::Error::Client(ClientSideError::NotFound(
					NotFoundError::LatestSyncMessageTime,
				))) => DateTime::<Utc>::from(UNIX_EPOCH),

				Err(e) => return Err(e.into()),
			};

			Ok(datetime_to_timestamp(latest_time))
		}
	}
}

async fn encrypt_messages(
	secret_key: &SecretKey,
	rng: &mut CryptoRng,
	messages_bytes: Vec<u8>,
) -> Result<Vec<u8>, Error> {
	if messages_bytes.len() <= EncryptedBlock::PLAIN_TEXT_SIZE {
		let mut nonce_and_cipher_text = Vec::with_capacity(OneShotEncryption::cipher_text_size(
			secret_key,
			messages_bytes.len(),
		));

		let EncryptedBlock { nonce, cipher_text } =
			OneShotEncryption::encrypt(secret_key, messages_bytes.as_slice(), rng)
				.map_err(Error::Encrypt)?;

		nonce_and_cipher_text.extend_from_slice(nonce.as_slice());
		nonce_and_cipher_text.extend(&cipher_text);

		Ok(nonce_and_cipher_text)
	} else {
		let mut rng = CryptoRng::from_seed(rng.generate_fixed());
		let mut nonce_and_cipher_text = Vec::with_capacity(StreamEncryption::cipher_text_size(
			secret_key,
			messages_bytes.len(),
		));

		let (nonce, cipher_stream) =
			StreamEncryption::encrypt(secret_key, messages_bytes.as_slice(), &mut rng);

		nonce_and_cipher_text.extend_from_slice(nonce.as_slice());

		let mut cipher_stream = pin!(cipher_stream);

		while let Some(ciphered_chunk) = cipher_stream.try_next().await.map_err(Error::Encrypt)? {
			nonce_and_cipher_text.extend(ciphered_chunk);
		}

		Ok(nonce_and_cipher_text)
	}
}

async fn wait_notification(mut rx: broadcast::Receiver<SyncEvent>) -> RaceNotifiedOrStopped {
	// wait until Created message comes in
	loop {
		if matches!(rx.recv().await, Ok(SyncEvent::Created)) {
			break;
		};
	}

	RaceNotifiedOrStopped::Notified
}
