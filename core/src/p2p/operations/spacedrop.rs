use std::{
	borrow::Cow,
	path::PathBuf,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use futures::future::join_all;
use sd_p2p::{
	spaceblock::{BlockSize, Range, SpaceblockRequest, SpaceblockRequests, Transfer},
	PeerId,
};
use tokio::{
	fs::File,
	io::{AsyncReadExt, AsyncWriteExt, BufReader},
	time::{sleep, Instant},
};
use tracing::debug;
use uuid::Uuid;

use crate::p2p::{Header, P2PEvent, P2PManager};

/// The amount of time to wait for a Spacedrop request to be accepted or rejected before it's automatically rejected
pub(crate) const SPACEDROP_TIMEOUT: Duration = Duration::from_secs(60);

// TODO: Proper error handling
pub async fn spacedrop(
	p2p: Arc<P2PManager>,
	// TODO: Stop using `PeerId`
	peer_id: PeerId,
	paths: Vec<PathBuf>,
) -> Result<Uuid, ()> {
	if paths.is_empty() {
		return Err(());
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
	.map_err(|_| ())? // TODO: Error handling
	.into_iter()
	.unzip();

	let total_length: u64 = requests.iter().map(|req| req.size).sum();

	let id = Uuid::new_v4();
	debug!("({id}): starting Spacedrop with peer '{peer_id}");
	let mut stream = p2p.manager.stream(peer_id).await.map_err(|err| {
		debug!("({id}): failed to connect: {err:?}");
		// TODO: Proper error
	})?;

	tokio::spawn(async move {
		debug!("({id}): connected, sending header");
		let header = Header::Spacedrop(SpaceblockRequests {
			id,
			block_size: BlockSize::from_size(total_length),
			requests,
		});
		if let Err(err) = stream.write_all(&header.to_bytes()).await {
			debug!("({id}): failed to send header: {err}");
			return;
		}
		let Header::Spacedrop(requests) = header else {
			unreachable!();
		};

		debug!("({id}): waiting for response");
		let result = tokio::select! {
		  result = stream.read_u8() => result,
		  // Add 5 seconds incase the user responded on the deadline and slow network
		   _ = sleep(SPACEDROP_TIMEOUT + Duration::from_secs(5)) => {
				debug!("({id}): timed out, cancelling");
				p2p.events.0.send(P2PEvent::SpacedropTimedout { id }).ok();
				return;
			},
		};

		match result {
			Ok(0) => {
				debug!("({id}): Spacedrop was rejected from peer '{peer_id}'");
				p2p.events.0.send(P2PEvent::SpacedropRejected { id }).ok();
				return;
			}
			Ok(1) => {}        // Okay
			Ok(_) => todo!(),  // TODO: Proper error
			Err(_) => todo!(), // TODO: Proper error
		}

		let cancelled = Arc::new(AtomicBool::new(false));
		p2p.spacedrop_cancelations
			.lock()
			.await
			.insert(id, cancelled.clone());

		debug!("({id}): starting transfer");
		let i = Instant::now();

		let mut transfer = Transfer::new(
			&requests,
			|percent| {
				p2p.events
					.0
					.send(P2PEvent::SpacedropProgress { id, percent })
					.ok();
			},
			&cancelled,
		);

		for (file_id, (path, file)) in files.into_iter().enumerate() {
			debug!("({id}): transmitting '{file_id}' from '{path:?}'");
			let file = BufReader::new(file);
			transfer.send(&mut stream, file).await;
		}

		debug!("({id}): finished; took '{:?}", i.elapsed());
	});

	Ok(id)
}

// TODO: Move these off the manager
impl P2PManager {
	pub async fn accept_spacedrop(&self, id: Uuid, path: String) {
		if let Some(chan) = self.spacedrop_pairing_reqs.lock().await.remove(&id) {
			chan.send(Some(path)).unwrap(); // TODO: will fail if timed out
		}
	}

	pub async fn reject_spacedrop(&self, id: Uuid) {
		if let Some(chan) = self.spacedrop_pairing_reqs.lock().await.remove(&id) {
			chan.send(None).unwrap();
		}
	}

	pub async fn cancel_spacedrop(&self, id: Uuid) {
		if let Some(cancelled) = self.spacedrop_cancelations.lock().await.remove(&id) {
			cancelled.store(true, Ordering::Relaxed);
		}
	}
}
