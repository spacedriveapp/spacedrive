use crate::{CloudServices, Error, KeyManager};

use sd_cloud_schema::{
	devices,
	sync::{
		groups,
		messages::{pull, MessagesCollection},
	},
	Client, Request, Response,
};
use sd_core_sync::{
	cloud_crdt_op_db, CRDTOperation, CompressedCRDTOperationsPerModel, SyncManager,
};

use sd_actors::{Actor, Stopper};
use sd_crypto::{
	cloud::{OneShotDecryption, SecretKey, StreamDecryption},
	primitives::{EncryptedBlock, StreamNonce},
};
use sd_prisma::prisma::PrismaClient;

use std::{
	collections::{hash_map::Entry, HashMap},
	future::IntoFuture,
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use chrono::{DateTime, Utc};
use futures::{FutureExt, StreamExt};
use futures_concurrency::future::{Race, TryJoin};
use quic_rpc::transport::quinn::QuinnConnection;
use serde::{Deserialize, Serialize};
use tokio::{fs, io, sync::Notify, time::sleep};
use tracing::{debug, error, instrument, warn};
use uuid::Uuid;

use super::{ReceiveAndIngestNotifiers, SyncActors, ONE_MINUTE};

const CLOUD_SYNC_DATA_KEEPER_DIRECTORY: &str = "cloud_sync_data_keeper";

/// Responsible for downloading sync operations from the cloud to be processed by the ingester

pub struct Receiver {
	keeper: LastTimestampKeeper,
	sync_group_pub_id: groups::PubId,
	device_pub_id: devices::PubId,
	cloud_services: Arc<CloudServices>,
	cloud_client: Client<QuinnConnection<Response, Request>>,
	key_manager: Arc<KeyManager>,
	sync: SyncManager,
	notifiers: Arc<ReceiveAndIngestNotifiers>,
	active: Arc<AtomicBool>,
	active_notifier: Arc<Notify>,
}

impl Actor<SyncActors> for Receiver {
	const IDENTIFIER: SyncActors = SyncActors::Receiver;

	async fn run(&mut self, stop: Stopper) {
		enum Race {
			Continue,
			Stop,
		}

		loop {
			self.active.store(true, Ordering::Relaxed);
			self.active_notifier.notify_waiters();

			let res = self.run_loop_iteration().await;

			self.active.store(false, Ordering::Relaxed);

			if let Err(e) = res {
				error!(?e, "Error during cloud sync receiver actor iteration");
				sleep(ONE_MINUTE).await;
				continue;
			}

			self.active_notifier.notify_waiters();

			if matches!(
				(
					sleep(ONE_MINUTE).map(|()| Race::Continue),
					self.notifiers
						.wait_notification_to_receive()
						.map(|()| Race::Continue),
					stop.into_future().map(|()| Race::Stop),
				)
					.race()
					.await,
				Race::Stop
			) {
				break;
			}
		}
	}
}

impl Receiver {
	pub async fn new(
		data_dir: impl AsRef<Path> + Send,
		sync_group_pub_id: groups::PubId,
		cloud_services: Arc<CloudServices>,
		sync: SyncManager,
		notifiers: Arc<ReceiveAndIngestNotifiers>,
		active: Arc<AtomicBool>,
		active_notify: Arc<Notify>,
	) -> Result<Self, Error> {
		let (keeper, cloud_client, key_manager) = (
			LastTimestampKeeper::load(data_dir.as_ref(), sync_group_pub_id),
			cloud_services.client(),
			cloud_services.key_manager(),
		)
			.try_join()
			.await?;

		Ok(Self {
			keeper,
			sync_group_pub_id,
			device_pub_id: devices::PubId(Uuid::from(&sync.device_pub_id)),
			cloud_services,
			cloud_client,
			key_manager,
			sync,
			notifiers,
			active,
			active_notifier: active_notify,
		})
	}

	async fn run_loop_iteration(&mut self) -> Result<(), Error> {
		let mut responses_stream = self
			.cloud_client
			.sync()
			.messages()
			.pull(pull::Request {
				access_token: self
					.cloud_services
					.token_refresher
					.get_access_token()
					.await?,
				group_pub_id: self.sync_group_pub_id,
				current_device_pub_id: self.device_pub_id,
				start_time_per_device: self
					.keeper
					.timestamps
					.iter()
					.map(|(device_pub_id, timestamp)| (*device_pub_id, *timestamp))
					.collect(),
			})
			.await?;

		while let Some(new_messages_res) = responses_stream.next().await {
			let pull::Response(new_messages) = new_messages_res??;
			if new_messages.is_empty() {
				break;
			}

			self.handle_new_messages(new_messages).await?;
		}

		debug!("Finished sync messages receiver actor iteration");

		self.keeper.save().await
	}

	async fn handle_new_messages(
		&mut self,
		new_messages: Vec<MessagesCollection>,
	) -> Result<(), Error> {
		debug!(
			new_messages_collections_count = new_messages.len(),
			start_time = ?new_messages.first().map(|c| c.start_time),
			end_time = ?new_messages.first().map(|c| c.end_time),
			"Handling new sync messages collections",
		);

		for message in new_messages.into_iter().filter(|message| {
			if message.original_device_pub_id == self.device_pub_id {
				warn!("Received sync message from the current device, need to check backend, this is a bug!");
				false
			} else {
				true
			}
		}) {
			debug!(
				new_messages_count = message.operations_count,
				start_time = ?message.start_time,
				end_time = ?message.end_time,
				"Handling new sync messages",
			);

			let (device_pub_id, timestamp) = handle_single_message(
				self.sync_group_pub_id,
				message,
				&self.key_manager,
				&self.sync,
			)
			.await?;

			match self.keeper.timestamps.entry(device_pub_id) {
				Entry::Occupied(mut entry) => {
					if entry.get() < &timestamp {
						*entry.get_mut() = timestamp;
					}
				}

				Entry::Vacant(entry) => {
					entry.insert(timestamp);
				}
			}

			// To ingest after each sync message collection is received, we MUST download and
			// store the messages SEQUENTIALLY, otherwise we might ingest messages out of order
			// due to parallel downloads
			self.notifiers.notify_ingester();
		}

		Ok(())
	}
}

#[instrument(
	skip_all,
	fields(%sync_group_pub_id, %original_device_pub_id, operations_count, ?key_hash, %end_time),
)]
async fn handle_single_message(
	sync_group_pub_id: groups::PubId,
	MessagesCollection {
		original_device_pub_id,
		end_time,
		operations_count,
		key_hash,
		encrypted_messages,
		..
	}: MessagesCollection,
	key_manager: &KeyManager,
	sync: &SyncManager,
) -> Result<(devices::PubId, DateTime<Utc>), Error> {
	// FIXME(@fogodev): If we don't have the key hash, we need to fetch it from another device in the group if possible
	let Some(secret_key) = key_manager.get_key(sync_group_pub_id, &key_hash).await else {
		return Err(Error::MissingKeyHash);
	};

	debug!(
		size = encrypted_messages.len(),
		"Received encrypted sync messages collection"
	);

	let crdt_ops = decrypt_messages(encrypted_messages, secret_key, original_device_pub_id).await?;

	assert_eq!(
		crdt_ops.len(),
		operations_count as usize,
		"Sync messages count mismatch"
	);

	write_cloud_ops_to_db(crdt_ops, &sync.db).await?;

	Ok((original_device_pub_id, end_time))
}

#[instrument(skip(encrypted_messages, secret_key), fields(messages_size = %encrypted_messages.len()), err)]
async fn decrypt_messages(
	encrypted_messages: Vec<u8>,
	secret_key: SecretKey,
	devices::PubId(device_pub_id): devices::PubId,
) -> Result<Vec<CRDTOperation>, Error> {
	let plain_text = if encrypted_messages.len() <= EncryptedBlock::CIPHER_TEXT_SIZE {
		OneShotDecryption::decrypt(&secret_key, encrypted_messages.as_slice().into())
			.map_err(Error::Decrypt)?
	} else {
		let (nonce, cipher_text) = encrypted_messages.split_at(size_of::<StreamNonce>());

		let mut plain_text = Vec::with_capacity(cipher_text.len());

		StreamDecryption::decrypt(
			&secret_key,
			nonce.try_into().expect("we split the correct amount"),
			cipher_text,
			&mut plain_text,
		)
		.await
		.map_err(Error::Decrypt)?;

		plain_text
	};

	rmp_serde::from_slice::<CompressedCRDTOperationsPerModel>(&plain_text)
		.map(|compressed_ops| compressed_ops.into_ops(device_pub_id))
		.map_err(Error::DeserializationFailureToPullSyncMessages)
}

#[instrument(skip_all, err)]
pub async fn write_cloud_ops_to_db(
	ops: Vec<CRDTOperation>,
	db: &PrismaClient,
) -> Result<(), sd_core_sync::Error> {
	db._batch(
		ops.into_iter()
			.map(|op| cloud_crdt_op_db(&op).map(|op| op.to_query(db)))
			.collect::<Result<Vec<_>, _>>()?,
	)
	.await?;

	Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct LastTimestampKeeper {
	timestamps: HashMap<devices::PubId, DateTime<Utc>>,
	file_path: Box<Path>,
}

impl LastTimestampKeeper {
	async fn load(data_dir: &Path, sync_group_pub_id: groups::PubId) -> Result<Self, Error> {
		let cloud_sync_data_directory = data_dir.join(CLOUD_SYNC_DATA_KEEPER_DIRECTORY);

		fs::create_dir_all(&cloud_sync_data_directory)
			.await
			.map_err(Error::FailedToCreateTimestampKeepersDirectory)?;

		let file_path = cloud_sync_data_directory
			.join(format!("{sync_group_pub_id}.bin"))
			.into_boxed_path();

		match fs::read(&file_path).await {
			Ok(bytes) => Ok(Self {
				timestamps: rmp_serde::from_slice(&bytes)
					.map_err(Error::LastTimestampKeeperDeserialization)?,
				file_path,
			}),

			Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self {
				timestamps: HashMap::new(),
				file_path,
			}),

			Err(e) => Err(Error::FailedToReadLastTimestampKeeper(e)),
		}
	}

	async fn save(&self) -> Result<(), Error> {
		fs::write(
			&self.file_path,
			&rmp_serde::to_vec_named(&self.timestamps)
				.map_err(Error::LastTimestampKeeperSerialization)?,
		)
		.await
		.map_err(Error::FailedToWriteLastTimestampKeeper)
	}
}
