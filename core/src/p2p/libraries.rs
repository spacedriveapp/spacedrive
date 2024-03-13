use std::{collections::HashMap, sync::Arc};

use sd_p2p2::{
	flume::bounded, HookEvent, HookId, IdentityOrRemoteIdentity, PeerConnectionCandidate, P2P,
};
use tracing::error;

use crate::library::{Libraries, LibraryManagerEvent};

/// A P2P hook which integrates P2P into Spacedrive's library system.
///
/// This hooks is responsible for:
///  - injecting library peers into the P2P system so we can connect to them over internet.
///
pub fn libraries_hook(p2p: Arc<P2P>, libraries: Arc<Libraries>) -> HookId {
	let (tx, rx) = bounded(15);
	let hook_id = p2p.register_hook("sd-libraries-hook", tx);

	let handle = tokio::spawn(async move {
		if let Err(err) = libraries
			.rx
			.clone()
			.subscribe(|msg| {
				let p2p = p2p.clone();
				async move {
					match msg {
						LibraryManagerEvent::InstancesModified(library)
						| LibraryManagerEvent::Load(library) => {
							p2p.metadata_mut().insert(
								library.id.to_string(),
								library.identity.to_remote_identity().to_string(),
							);

							let Ok(instances) =
								library.db.instance().find_many(vec![]).exec().await
							else {
								return;
							};

							for i in instances.iter() {
								let identity = IdentityOrRemoteIdentity::from_bytes(&i.identity)
									.expect("lol: invalid DB entry")
									.remote_identity();

								p2p.clone().discover_peer(
									hook_id,
									identity,
									HashMap::new(), // TODO: We should probs cache this so we have something
									[PeerConnectionCandidate::Relay].into_iter().collect(),
								);
							}
						}
						LibraryManagerEvent::Edit(_library) => {
							// TODO: Send changes to all connected nodes or queue sending for when they are online!
						}
						LibraryManagerEvent::Delete(library) => {
							p2p.metadata_mut().remove(&library.id.to_string());

							let Ok(instances) =
								library.db.instance().find_many(vec![]).exec().await
							else {
								return;
							};

							for i in instances.iter() {
								let identity = IdentityOrRemoteIdentity::from_bytes(&i.identity)
									.expect("lol: invalid DB entry")
									.remote_identity();

								let peers = p2p.peers();
								let Some(peer) = peers.get(&identity) else {
									continue;
								};
								peer.undiscover_peer(hook_id);
							}
						}
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `LibraryServices::start` manager aborted with error: {err:?}");
		}
	});

	tokio::spawn(async move {
		while let Ok(event) = rx.recv_async().await {
			match event {
				HookEvent::Shutdown { _guard } => {
					handle.abort();
					break;
				}
				_ => continue,
			}
		}
	});

	hook_id
}
