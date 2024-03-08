use std::sync::Arc;

use sd_p2p2::P2P;
use tracing::error;

use crate::library::{Libraries, LibraryManagerEvent};

pub fn start(p2p: Arc<P2P>, libraries: Arc<Libraries>) {
	// TODO: Cleanup this thread on p2p shutdown.
	tokio::spawn(async move {
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

							// TODO
						}
						LibraryManagerEvent::Edit(_library) => {
							// TODO: Send changes to all connected nodes or queue sending for when they are online!
						}
						LibraryManagerEvent::Delete(library) => {
							p2p.metadata_mut().remove(&library.id.to_string());
						}
					}
				}
			})
			.await
		{
			error!("Core may become unstable! `LibraryServices::start` manager aborted with error: {err:?}");
		}
	});
}
