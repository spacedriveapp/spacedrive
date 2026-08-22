//! Ask for the tiles a client is about to draw.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::action::{error::ActionError, CoreAction},
	service::thumbs::{TileIdentity, TILE},
};

/// Paths in draw order. The first is the most urgent, so a client sends its
/// viewport before its prefetch margin.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbRequestInput {
	pub paths: Vec<SdPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbRequestOutput {
	/// The distinct sources these tiles live in. A client maps each file
	/// read-only and copies tiles straight out of it.
	pub sources: Vec<ThumbSource>,
	/// One per input path, in order. `None` where the path has no tile: it is
	/// under no registered source, or the file is gone.
	pub tiles: Vec<Option<TileIdentity>>,
}

/// Where one source's tiles are stored, and at what geometry.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbSource {
	pub id: Uuid,
	pub cache_path: PathBuf,
	/// Envelope edge in physical pixels: slots are square, but the frame inside
	/// one is the tile's own aspect and comes back with every read.
	pub tile_size: u32,
}

pub struct ThumbRequestAction {
	paths: Vec<PathBuf>,
}

impl CoreAction for ThumbRequestAction {
	type Input = ThumbRequestInput;
	type Output = ThumbRequestOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		// A tile is baked from bytes on this machine, so only a physical path
		// can name one. Anything else has no local file to decode.
		let paths = input
			.paths
			.into_iter()
			.map(|path| match path {
				SdPath::Physical { path, .. } => Ok(path),
				other => Err(format!("{other:?} does not name a local file")),
			})
			.collect::<Result<Vec<_>, _>>()?;
		Ok(Self { paths })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let tiles = context.thumbs.request(&self.paths).await;

		let mut seen = HashSet::new();
		let sources = tiles
			.iter()
			.flatten()
			.filter(|tile| seen.insert(tile.source_id))
			.filter_map(|tile| {
				Some(ThumbSource {
					id: tile.source_id,
					cache_path: context.thumbs.cache_path(tile.source_id)?,
					tile_size: TILE,
				})
			})
			.collect();

		Ok(ThumbRequestOutput { sources, tiles })
	}

	fn action_kind(&self) -> &'static str {
		"thumbs.request"
	}
}

crate::register_core_action!(ThumbRequestAction, "thumbs.request");
