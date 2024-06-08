use std::{
	borrow::Cow,
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, PoisonError,
	},
	time::Duration,
};

use crate::p2p::{Header, P2PEvent, P2PManager};
use futures::future::join_all;
use sd_p2p::{RemoteIdentity, UnicastStream};
use sd_p2p_block::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer};
use thiserror::Error;
use tokio::{
	fs::{create_dir_all, File},
	io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
	sync::oneshot,
	time::{sleep, Instant},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// The amount of time to wait for a Spacedrop request to be accepted or rejected before it's automatically rejected
pub(crate) const SPACEDROP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
pub enum SpacedropError {
	#[error("paths argument is an empty vector")]
	EmptyPath,
	#[error("error connecting to peer")]
	FailedPeerConnection,
	#[error("error creating stream: {0}")]
	FailedNewStream(#[from] sd_p2p::NewStreamError),
	#[error("error opening file: {0}")]
	FailedFileOpen(#[from] std::io::Error),
}

pub async fn spacedrop(
	p2p: Arc<P2PManager>,
	identity: RemoteIdentity,
	paths: Vec<PathBuf>,
) -> Result<Uuid, SpacedropError> {
	if paths.is_empty() {
		return Err(SpacedropError::EmptyPath);
	}

	let (files, requests): (Vec<_>, Vec<_>) = join_all(paths.into_iter().map(|path| async move {
		let file = File::open(&path).await?;
		let metadata = file.metadata().await?;
		let name = path
			.file_name()
			.map(|v| v.to_string_lossy())
			.unwrap_or(Cow::Borrowed(""))
			.to_string();

		Ok((
			(path, file),
			SpaceblockRequest {
				name,
				size: metadata.len(),
				range: Range::Full,
			},
		))
	}))
	.await
	.into_iter()
	.collect::<Result<Vec<_>, std::io::Error>>()
	.map_err(SpacedropError::FailedFileOpen)?
	.into_iter()
	.unzip();

	let total_length: u64 = requests.iter().map(|req| req.size).sum();

	let id = Uuid::new_v4();
	debug!(spacedrop_id = %id, peer = %identity, "Starting Spacedrop;");
	let peer = p2p
		.p2p
		.peers()
		.get(&identity)
		.ok_or_else(|| {
			debug!(spacedrop_id = %id, peer = %identity, "Failed to find connection method;");
			SpacedropError::FailedPeerConnection
		})?
		.clone();

	let mut stream = peer.new_stream().await.map_err(|e| {
		debug!(spacedrop_id = %id, peer = %identity, ?e, "Failed to connect");
		SpacedropError::FailedNewStream(e)
	})?;

	tokio::spawn(async move {
		debug!(spacedrop_id = %id, "Connected, sending header");
		let header = Header::Spacedrop(SpaceblockRequests {
			id,
			block_size: BlockSize::from_file_size(total_length),
			requests,
		});
		if let Err(e) = stream.write_all(&header.to_bytes()).await {
			debug!(spacedrop_id = %id, ?e, "Failed to send header");
			return;
		}
		let Header::Spacedrop(requests) = header else {
			unreachable!();
		};

		debug!(spacedrop_id = %id, "Waiting for response");
		let result = tokio::select! {
		  result = stream.read_u8() => result,
		  // Add 5 seconds incase the user responded on the deadline and slow network
		   _ = sleep(SPACEDROP_TIMEOUT + Duration::from_secs(5)) => {
				debug!(spacedrop_id = %id, "Timed out, cancelling");
				p2p.events.send(P2PEvent::SpacedropTimedOut { id }).ok();
				return;
			},
		};

		match result {
			Ok(0) => {
				debug!(spacedrop_id = %id, peer = %identity, "Spacedrop was rejected from;");
				p2p.events.send(P2PEvent::SpacedropRejected { id }).ok();
				return;
			}
			Ok(1) => {}                 // Okay
			Ok(_) => todo!(),           // TODO: Proper error
			Err(e) => todo!("{:?}", e), // TODO: Proper error
		}

		let cancelled = Arc::new(AtomicBool::new(false));
		p2p.spacedrop_cancellations
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.insert(id, cancelled.clone());

		debug!(spacedrop_id = %id, "Starting transfer");
		let i = Instant::now();

		let mut transfer = Transfer::new(
			&requests,
			|percent| {
				p2p.events
					.send(P2PEvent::SpacedropProgress { id, percent })
					.ok();
			},
			&cancelled,
		);

		for (file_id, (path, file)) in files.into_iter().enumerate() {
			debug!(
				spacedrop_id = %id,
				%file_id,
				path = %path.display(),
				"Transmitting;",
			);

			let file = BufReader::new(file);
			if let Err(e) = transfer.send(&mut stream, file).await {
				debug!(
					spacedrop_id = %id,
					%file_id,
					?e,
					"Failed to send file;");
				// TODO: Error to frontend
				// p2p.events
				// 	.send(P2PEvent::SpacedropFailed { id, file_id })
				// 	.ok();
				return;
			}
		}

		debug!(spacedrop_id = %id, elapsed_time = ?i.elapsed(), "Finished;");
	});

	Ok(id)
}

// TODO: Move these off the manager
impl P2PManager {
	pub async fn accept_spacedrop(&self, id: Uuid, path: String) {
		if let Some(chan) = self
			.spacedrop_pairing_reqs
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&id)
		{
			chan.send(Some(path))
				.map_err(|e| {
					warn!(spacedrop_id = %id, ?e, "Error accepting Spacedrop;");
				})
				.ok();
		}
	}

	pub async fn reject_spacedrop(&self, id: Uuid) {
		if let Some(chan) = self
			.spacedrop_pairing_reqs
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&id)
		{
			chan.send(None)
				.map_err(|e| {
					warn!(spacedrop_id = %id, ?e, "Error rejecting Spacedrop;");
				})
				.ok();
		}
	}

	pub async fn cancel_spacedrop(&self, id: Uuid) {
		if let Some(cancelled) = self
			.spacedrop_cancellations
			.lock()
			.unwrap_or_else(PoisonError::into_inner)
			.remove(&id)
		{
			cancelled.store(true, Ordering::Relaxed);
		}
	}
}

pub(crate) async fn receiver(
	this: &Arc<P2PManager>,
	req: SpaceblockRequests,
	mut stream: UnicastStream,
) -> Result<(), ()> {
	let id = req.id;
	let (tx, rx) = oneshot::channel();

	info!(
		spacedrop_id = %id,
		files_count = req.requests.len(),
		peer = %stream.remote_identity(),
		block_size = ?req.block_size,
		"Receiving spacedrop files;",
	);
	this.spacedrop_pairing_reqs
		.lock()
		.unwrap_or_else(PoisonError::into_inner)
		.insert(id, tx);

	if this
		.events
		.send(P2PEvent::SpacedropRequest {
			id,
			identity: stream.remote_identity(),
			peer_name: "Unknown".into(),
			// TODO: A better solution to this
			// manager
			// 	.get_discovered_peers()
			// 	.await
			// 	.into_iter()
			// 	.find(|p| p.peer_id == event.peer_id)
			// 	.map(|p| p.metadata.name)
			// 	.unwrap_or_else(|| "Unknown".to_string()),
			files: req
				.requests
				.iter()
				.map(|req| req.name.clone())
				.collect::<Vec<_>>(),
		})
		.is_err()
	{
		// No frontend's are active

		// TODO: Implement this
		error!("TODO: Outright reject Spacedrop");
	}

	tokio::select! {
		_ = sleep(SPACEDROP_TIMEOUT) => {
			info!(spacedrop_id = %id, "Timeout, rejecting!;");

			stream.write_all(&[0]).await.map_err(|e| {
				error!(spacedrop_id = %id, ?e, "Error reject bit;");
			})?;
			stream.flush().await.map_err(|e| {
				error!(spacedrop_id = %id, ?e, "Error flushing reject bit;");
			})?;
		}
		file_path = rx => {
			match file_path {
				Ok(Some(file_path)) => {
					info!(spacedrop_id = %id, saving_to = %file_path, "Accepted;");

					let cancelled = Arc::new(AtomicBool::new(false));
					this.spacedrop_cancellations
						.lock()
						.unwrap_or_else(PoisonError::into_inner)
						.insert(id, cancelled.clone());

					stream.write_all(&[1]).await.map_err(|e| {
						error!(spacedrop_id = %id, ?e, "Error sending continuation bit;");

						// TODO: Send error to the frontend

						// TODO: make sure the other peer times out or we retry???
					})?;

					let names = req.requests.iter().map(|req| req.name.clone()).collect::<Vec<_>>();
					let mut transfer = Transfer::new(&req, |percent| {
						this.events.send(P2PEvent::SpacedropProgress { id, percent }).ok();
					}, &cancelled);

					let file_path = PathBuf::from(file_path);
					let names_len = names.len();
					for file_name in names {
						 // When transferring more than 1 file we wanna join the incoming file name to the directory provided by the user
						 let mut path = file_path.clone();
						 if names_len != 1 {
							// We know the `file_path` will be a directory so we can just push the file name to it
							path.push(&file_name);
						}

						debug!(
							spacedrop_id = %id,
							%file_name,
							saving_to = %path.display(),
							"Accepting;",
						);

						if let Some(parent) = path.parent() {
						  create_dir_all(&parent).await.map_err(|e| {
								error!(
									spacedrop_id = %id,
									parent = %parent.display(),
									?e,
									"Error creating parent directory;");

								// TODO: Send error to the frontend

								// TODO: Send error to remote peer
							})?;
						}

						let f = File::create(&path).await.map_err(|e| {
							error!(
								spacedrop_id = %id,
								creating_file_at = %path.display(),
								?e,
								"Error creating file;",
							);

							// TODO: Send error to the frontend

							// TODO: Send error to remote peer
						})?;
						let f = BufWriter::new(f);
						if let Err(e) = transfer.receive(&mut stream, f).await {
							error!(
								spacedrop_id = %id,
								%file_name,
								?e,
								"Error receiving file;");

							// TODO: Send error to frontend

							break;
						}
					}

					info!(spacedrop_id = %id, "Completed;");
				}
				Ok(None) => {
					info!(spacedrop_id = %id, "Rejected;");

					stream.write_all(&[0]).await.map_err(|e| {
					   error!(spacedrop_id = %id, ?e, "Error sending rejection;");
					})?;
					stream.flush().await.map_err(|e| {
					   error!(spacedrop_id = %id, ?e, "Error flushing rejection;");
					})?;
				}
				Err(_) => {
					warn!(spacedrop_id = %id, "Error with Spacedrop pairing request receiver!;");
				}
			}
		}
	};

	Ok(())
}
