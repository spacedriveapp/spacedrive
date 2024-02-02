use std::sync::Arc;

use sd_p2p2::P2P;
use tracing::error;

use crate::library::{Libraries, Library, LibraryManagerEvent};

pub fn start(p2p: Arc<P2P>, libraries: Arc<Libraries>) {
	tokio::spawn(async move {
		if let Err(err) = libraries
			.rx
			.clone()
			.subscribe(|msg| {
				let p2p = p2p.clone();
				async move {
					match msg {
						LibraryManagerEvent::InstancesModified(library)
						| LibraryManagerEvent::Load(library) => on_load(&p2p, &library).await,
						LibraryManagerEvent::Edit(_library) => {
							// TODO: Send changes to all connected nodes!
							// TODO: Update mdns
						}
						LibraryManagerEvent::Delete(_library) => {
							// TODO: Remove library
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

async fn on_load(p2p: &P2P, library: &Library) {
	let mut service = p2p.metadata_mut();
	service.insert(
		library.id.to_string(),
		library.identity.to_remote_identity().to_string(),
	);
}
