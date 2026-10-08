//! Naming stored content rows after the kinds loaded extensions declare.
//!
//! The content identity phase writes a row's kind when it reads the bytes.
//! Rows identified before an extension existed carry a built-in kind, or
//! nothing at all, and the bytes are not read again for them: one update
//! per declared kind names every row whose file carries one of the kind's
//! extensions. The pass runs when a store opens, so every store sees the
//! kinds loaded at startup, and when an extension with kinds loads while
//! stores are open. It is idempotent, so running it twice costs one indexed
//! probe per kind and changes nothing.

use std::sync::Arc;

use crate::filetype::FileTypeRegistry;
use crate::ops::indexing::VolumeIndex;

/// Name the rows of one store after every extension kind the registry
/// holds. Returns how many rows gained a name.
pub async fn name_kinds_in_store(
	db: &sd_store::SourceDb,
	registry: &FileTypeRegistry,
) -> Result<u64, sd_store::Error> {
	let mut named = 0;
	for (kind, extensions) in registry.extension_kinds() {
		let Some(kind_name) = kind.kind_name.as_deref() else {
			continue;
		};
		named += sd_store::name_content_kind_by_extension(
			db.pool(),
			kind_name,
			kind.category as i32 as i64,
			&extensions,
		)
		.await?;
	}
	Ok(named)
}

/// Run the pass over every store this machine has open.
pub async fn name_kinds_in_open_stores(volume_index: &Arc<VolumeIndex>) {
	let registry = FileTypeRegistry::current();
	if registry.extension_kinds().is_empty() {
		return;
	}
	for store in volume_index.open_stores() {
		match name_kinds_in_store(store.db(), &registry).await {
			Ok(0) => {}
			Ok(named) => {
				tracing::info!(source = %store.id(), named, "named content rows after extension kinds")
			}
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "could not name content rows after extension kinds")
			}
		}
	}
}
