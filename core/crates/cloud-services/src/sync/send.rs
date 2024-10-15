use crate::{CloudServices, Error, KeyManager};

use sd_core_sync::{CompressedCRDTOperationsPerModelPerDevice, SyncEvent, SyncManager, NTP64};

use sd_actors::{Actor, Stopper};
use sd_cloud_schema::{
	devices,
	error::{ClientSideError, NotFoundError},
	sync::{self, groups, messages},
	Client, Service,
};
use sd_crypto::{
	cloud::{OneShotEncryption, SecretKey, StreamEncryption},
	primitives::EncryptedBlock,
	CryptoRng, SeedableRng,
};
use sd_utils::{datetime_to_timestamp, timestamp_to_datetime};

use std::{
	future::IntoFuture,
	num::NonZero,
	pin::{pin, Pin},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::{Duration, UNIX_EPOCH},
};

use async_stream::try_stream;
use chrono::{DateTime, Utc};
use futures::{FutureExt, SinkExt, Stream, StreamExt, TryStream, TryStreamExt};
use futures_concurrency::future::{Race, TryJoin};
use quic_rpc::{client::UpdateSink, pattern::bidi_streaming, transport::quinn::QuinnConnection};
use reqwest_middleware::reqwest::{header, Body};
use tokio::{
	spawn,
	sync::{broadcast, oneshot, Notify, Semaphore},
	time::sleep,
};
use tracing::{debug, error};
use uuid::Uuid;

use super::{SyncActors, ONE_MINUTE};

const TEN_SECONDS: Duration = Duration::from_secs(10);
const THIRTY_SECONDS: Duration = Duration::from_secs(30);

const MESSAGES_COLLECTION_SIZE: u32 = 100_000;

enum RaceNotifiedOrStopped {
	Notified,
	Stopped,
}

enum LoopStatus {
	SentMessages,
	Idle,
}

type LatestTimestamp = NTP64;

type PushResponsesStream = Pin<
	Box<
		dyn Stream<
				Item = Result<
					Result<messages::push::Response, sd_cloud_schema::Error>,
					bidi_streaming::ItemError<QuinnConnection<Service>>,
				>,
			> + Send
			+ Sync,
	>,
>;

#[derive(Debug)]
pub struct Sender {
	sync_group_pub_id: groups::PubId,
	sync: SyncManager,
	cloud_services: Arc<CloudServices>,
	cloud_client: Client<QuinnConnection<Service>>,
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
		while let Some(ops_res) = crdt_ops_stream.next().await {
			let ops = ops_res?;

			let (Some(first), Some(last)) = (ops.first(), ops.last()) else {
				break;
			};

			#[allow(clippy::cast_possible_truncation)]
			let operations_count = ops.len() as u32;

			new_latest_timestamp = last.timestamp;

			let start_time = timestamp_to_datetime(first.timestamp);
			let end_time = timestamp_to_datetime(last.timestamp);

			// Ignoring this device_pub_id here as we already know it
			let (_device_pub_id, compressed_ops) =
				CompressedCRDTOperationsPerModelPerDevice::new_single_device(ops);

			let messages_bytes = rmp_serde::to_vec_named(&compressed_ops)
				.map_err(Error::SerializationFailureToPushSyncMessages)?;

			let plain_text_size = messages_bytes.len();
			let expected_blob_size = if plain_text_size <= EncryptedBlock::PLAIN_TEXT_SIZE {
				OneShotEncryption::cipher_text_size(&secret_key, plain_text_size)
			} else {
				StreamEncryption::cipher_text_size(&secret_key, plain_text_size)
			} as u64;

			debug!(?expected_blob_size, ?key_hash, "Preparing sync message");

			let (mut push_updates, mut push_responses) = self
				.cloud_client
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
					start_time,
					end_time,
					expected_blob_size,
				})
				.await?;

			let Some(response) = push_responses.next().await else {
				return Err(Error::EmptyResponse("push initial response"));
			};

			let messages::push::Response(response_kind) = response??;

			match response_kind {
				messages::push::ResponseKind::SinglePresignedUrl(url) => {
					upload_to_single_url(
						url,
						secret_key.clone(),
						self.cloud_services.http_client(),
						messages_bytes,
						&mut self.rng,
					)
					.await?;
				}
				messages::push::ResponseKind::ManyPresignedUrls(urls) => {
					upload_to_many_urls(
						urls,
						secret_key.clone(),
						self.cloud_services.http_client().clone(),
						messages_bytes,
						&mut self.rng,
						&mut push_updates,
						&mut push_responses,
					)
					.await?;
				}
				messages::push::ResponseKind::Pong => {
					return Err(Error::UnexpectedResponse(
						"Pong on first messages push request",
					))
				}
				messages::push::ResponseKind::End => {
					return Err(Error::UnexpectedResponse(
						"End on first messages push request",
					))
				}
			}

			finalize_protocol(&mut push_updates, &mut push_responses).await?;

			status = LoopStatus::SentMessages;
		}

		self.maybe_latest_timestamp = Some(new_latest_timestamp);

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
					current_device_pub_id,
					kind: messages::get_latest_time::Kind::ForCurrentDevice,
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

async fn finalize_protocol(
	push_updates: &mut UpdateSink<
		Service,
		QuinnConnection<Service>,
		messages::push::RequestUpdate,
		sync::Service,
	>,
	push_responses: &mut PushResponsesStream,
) -> Result<(), Error> {
	push_updates
		.send(messages::push::RequestUpdate(
			messages::push::UpdateKind::End,
		))
		.await
		.map_err(Error::EndUpdatePushSyncMessages)?;

	let Some(response) = push_responses.next().await else {
		return Err(Error::EmptyResponse("push initial response"));
	};

	let messages::push::Response(response_kind) = response??;

	match response_kind {
		messages::push::ResponseKind::SinglePresignedUrl(_)
		| messages::push::ResponseKind::ManyPresignedUrls(_) => {
			return Err(Error::UnexpectedResponse(
				"Urls responses on final messages push response",
			))
		}
		messages::push::ResponseKind::Pong => {
			return Err(Error::UnexpectedResponse(
				"Pong on final message push response",
			))
		}
		messages::push::ResponseKind::End => {
			/*
			   Everything is awesome!
			*/
		}
	}

	Ok(())
}

async fn upload_to_many_urls(
	urls: Vec<reqwest::Url>,
	secret_key: SecretKey,
	http_client: reqwest_middleware::ClientWithMiddleware,
	messages_bytes: Vec<u8>,
	rng: &mut CryptoRng,
	push_updates: &mut UpdateSink<
		Service,
		QuinnConnection<Service>,
		messages::push::RequestUpdate,
		sync::Service,
	>,
	push_responses: &mut PushResponsesStream,
) -> Result<(), Error> {
	let stop_ping_pong = Arc::new(AtomicBool::new(false));
	let (out_tx, mut out_rx) = oneshot::channel();
	let rng = CryptoRng::from_seed(rng.generate_fixed());

	let handle = spawn(handle_multipart_upload(
		urls,
		secret_key,
		http_client,
		messages_bytes,
		rng,
		Arc::clone(&stop_ping_pong),
		out_tx,
	));

	loop {
		if stop_ping_pong.load(Ordering::Acquire) {
			break;
		}

		if let Err(e) = push_updates
			.send(messages::push::RequestUpdate(
				messages::push::UpdateKind::Ping,
			))
			.await
		{
			error!(?e, "Failed to send push ping update");
			sleep(TEN_SECONDS).await;
			continue;
		}

		let Some(response) = push_responses.next().await else {
			error!("Empty response from push ping response");
			continue;
		};

		match response {
			Ok(Ok(messages::push::Response(
				messages::push::ResponseKind::SinglePresignedUrl(_)
				| messages::push::ResponseKind::ManyPresignedUrls(_),
			))) => {
				unreachable!("can't receive url if we didn't send an initial request")
			}

			Ok(Ok(messages::push::Response(messages::push::ResponseKind::Pong))) => {
				/*
				   Everything is awesome!
				*/
			}
			Ok(Ok(messages::push::Response(messages::push::ResponseKind::End))) => {
				unreachable!("Can't receive an End if we didn't send an End first");
			}

			Ok(Err(e)) => {
				error!(?e, "Error from push ping response");
				sleep(TEN_SECONDS).await;
				continue;
			}

			Err(e) => {
				error!(?e, "Error from push ping response");
				sleep(TEN_SECONDS).await;
				continue;
			}
		}

		if stop_ping_pong.load(Ordering::Acquire) {
			break;
		}

		sleep(THIRTY_SECONDS).await;
	}

	let Ok(out) = out_rx.try_recv() else {
		// SAFETY: This try_recv error can only happen if the upload task panicked
		// so we're good to unwrap the error.
		let e = handle.await.expect_err("upload task panicked");
		error!(?e, "Critical error while uploading sync messages");
		return Err(Error::CriticalErrorWhileUploadingSyncMessages);
	};

	out
}

async fn handle_multipart_upload(
	urls: Vec<reqwest::Url>,
	secret_key: SecretKey,
	http_client: reqwest_middleware::ClientWithMiddleware,
	messages_bytes: Vec<u8>,
	rng: CryptoRng,
	stop_ping_pong: Arc<AtomicBool>,
	out_tx: oneshot::Sender<Result<(), Error>>,
) {
	async fn inner(
		urls: Vec<reqwest::Url>,
		secret_key: SecretKey,
		http_client: reqwest_middleware::ClientWithMiddleware,
		messages_bytes: Vec<u8>,
		mut rng: CryptoRng,
	) -> Result<(), Error> {
		let urls_count = urls.len();
		let message_size = messages_bytes.len();
		let blocks_per_url = message_size / urls_count / EncryptedBlock::PLAIN_TEXT_SIZE;
		let cipher_text_size = StreamEncryption::cipher_text_size(&secret_key, message_size);

		let parallel_upload_semaphore = Arc::new(Semaphore::new(
			std::thread::available_parallelism()
				.map(NonZero::get)
				.unwrap_or(1),
		));

		// If we're uploading to many URLs, it implies that the message size is bigger than a single
		// encryption block, so we always use stream encryption.

		let mut buffers = vec![Vec::with_capacity(cipher_text_size / urls_count); urls_count];
		let (nonce, cipher_stream) =
			StreamEncryption::encrypt(&secret_key, messages_bytes.as_slice(), &mut rng);

		buffers[0].extend_from_slice(&nonce);

		let mut cipher_stream = pin!(cipher_stream);

		let mut handles = Vec::with_capacity(urls_count);

		for (idx, (mut buffer, url)) in buffers.into_iter().zip(urls).enumerate() {
			for _ in 0..blocks_per_url {
				if let Some(cipher_res) = cipher_stream.next().await {
					buffer.extend(cipher_res.map_err(Error::Encrypt)?);
				} else {
					return Err(Error::UnexpectedEndOfStream);
				}
			}

			handles.push(spawn(upload_part(
				idx,
				url,
				http_client.clone(),
				buffer,
				Arc::clone(&parallel_upload_semaphore),
			)));
		}

		assert!(
			cipher_stream.next().await.is_none(),
			"Unexpected ciphered bytes still on stream"
		);

		handles.try_join().await.map_err(|e| {
			error!(?e, "Error while uploading sync messages");
			Error::CriticalErrorWhileUploadingSyncMessages
		})?;

		Ok(())
	}

	let res = inner(urls, secret_key, http_client, messages_bytes, rng).await;
	stop_ping_pong.store(true, Ordering::Release);
	out_tx
		.send(res)
		.expect("upload output channel never closes");
}

async fn upload_part(
	idx: usize,
	url: reqwest::Url,
	http_client: reqwest_middleware::ClientWithMiddleware,
	buffer: Vec<u8>,
	parallel_upload_semaphore: Arc<Semaphore>,
) -> Result<(), Error> {
	let _permit = parallel_upload_semaphore
		.acquire()
		.await
		.expect("Semaphore never closes");

	let response = http_client
		.put(url)
		.header(header::CONTENT_LENGTH, buffer.len())
		.body(buffer)
		.send()
		.await
		.map_err(Error::UploadSyncMessages)?
		.error_for_status()
		.map_err(Error::ErrorResponseUploadSyncMessages)?;

	debug!(?response, idx, "Uploaded sync messages part");

	Ok(())
}

async fn upload_to_single_url(
	url: reqwest::Url,
	secret_key: SecretKey,
	http_client: &reqwest_middleware::ClientWithMiddleware,
	messages_bytes: Vec<u8>,
	rng: &mut CryptoRng,
) -> Result<(), Error> {
	let (cipher_text_size, body) = if messages_bytes.len() <= EncryptedBlock::PLAIN_TEXT_SIZE {
		let EncryptedBlock { nonce, cipher_text } =
			OneShotEncryption::encrypt(&secret_key, messages_bytes.as_slice(), rng)
				.map_err(Error::Encrypt)?;

		let cipher_text_size = nonce.len() + cipher_text.len();

		let mut body_bytes = Vec::with_capacity(cipher_text_size);
		body_bytes.extend_from_slice(nonce.as_slice());
		body_bytes.extend(&cipher_text);

		(cipher_text_size, Body::from(body_bytes))
	} else {
		let mut rng = CryptoRng::from_seed(rng.generate_fixed());
		let cipher_text_size =
			StreamEncryption::cipher_text_size(&secret_key, messages_bytes.len());

		let body_bytes = stream_encryption(secret_key, messages_bytes, &mut rng)
			.try_fold(
				Vec::with_capacity(cipher_text_size),
				|mut body_bytes, ciphered_chunk| async move {
					body_bytes.extend(ciphered_chunk);
					Ok(body_bytes)
				},
			)
			.await?;

		(cipher_text_size, Body::from(body_bytes))
	};

	http_client
		.put(url)
		.header(header::CONTENT_LENGTH, cipher_text_size)
		.body(body)
		.send()
		.await
		.map_err(Error::UploadSyncMessages)?
		.error_for_status()
		.map_err(Error::ErrorResponseUploadSyncMessages)?;

	Ok(())
}

fn stream_encryption(
	secret_key: SecretKey,
	messages_bytes: Vec<u8>,
	rng: &mut CryptoRng,
) -> impl TryStream<Ok = Vec<u8>, Error = Error> + Send + 'static {
	let mut rng = CryptoRng::from_seed(rng.generate_fixed());

	try_stream! {
		let (nonce, cipher_stream) =
		StreamEncryption::encrypt(&secret_key, messages_bytes.as_slice(), &mut rng);

		let mut cipher_stream = pin!(cipher_stream);

		yield nonce.to_vec();

		while let Some(res) = cipher_stream.next().await {
			yield res.map_err(Error::Encrypt)?;
		}
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
