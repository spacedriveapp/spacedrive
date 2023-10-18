use std::{fmt, sync::Arc};

use arc_swap::ArcSwap;
use tokio::sync::{mpsc, OnceCell};
use tracing::warn;

use crate::Metadata;

/// is a wrapper around `ArcSwap` and provides an API for the application to update the metadata about the current device.
/// This wrapper exists to ensure we ask the MDNS service to re-advertise the new metadata on change.
#[deprecated]
pub struct MetadataManager<TMeta: Metadata>(
	ArcSwap<TMeta>,
	// Starts out `None` cause this is constructed in userspace but when passed into `Manager::new` this will be set.
	OnceCell<mpsc::UnboundedSender<()>>,
);

impl<TMetdata: Metadata + fmt::Debug> fmt::Debug for MetadataManager<TMetdata> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MetadataManager")
			.field("metadata", &self.0.load())
			.finish()
	}
}

impl<TMeta: Metadata> MetadataManager<TMeta> {
	pub fn new(metadata: TMeta) -> Arc<Self> {
		Arc::new(Self(ArcSwap::new(Arc::new(metadata)), OnceCell::default()))
	}

	pub(crate) async fn set_tx(&self, tx: mpsc::UnboundedSender<()>) {
		self.1.get_or_init(move || async move { tx }).await;
	}

	/// Returns a copy of the current metadata
	pub fn get(&self) -> TMeta {
		TMeta::clone(&self.0.load())
	}

	/// Updates the metadata and asks the MDNS service to re-advertise the new metadata
	pub fn update(&self, metadata: TMeta) {
		self.0.store(Arc::new(metadata));
		if let Some(chan) = self.1.get() {
			chan.send(())
				.map_err(|_| {
					warn!("'MetadataManager' failed to ask the MDNS server to re-advertise!");
				})
				.ok();
		}
	}
}
