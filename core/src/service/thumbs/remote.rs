//! # Tiles from a peer
//!
//! A replica lists a paired device's files but cannot read them, so it cannot
//! bake their tiles. The device that owns the files already has, and this
//! module moves those tiles across. A client asks for a remote file's tile the
//! way it asks for a local one: the daemon answers at once with the version it
//! holds, zero when none, and fetches anything missing or unconfirmed from the
//! owner in the background. Each tile that lands goes into a local cache keyed
//! by the owner's volume, the same `volumes/<id>/thumbs.pvcache` layout every
//! client already reads, and is announced with the owner's version, which
//! clients adopt.
//!
//! The owner's version is stored as given. It covers the owner's decoders and
//! the file's full-precision mtime, neither of which a replica can see, so a
//! version derived here would disagree and read every tile as stale.
//!
//! Tiles cross the wire as lossy WebP. A raw slot is 576 KB, and a thumbnail
//! is already a downscale shown at or below its own size.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use sd_pvcache::Frame;
use tracing::debug;
use uuid::Uuid;

use crate::context::CoreContext;
use crate::ops::indexing::state::EntryKind;
use crate::service::mounts::peer;
use crate::service::network::protocol::byterange::{TileAnswer, TileAsk, MAX_REQUEST_FRAME};

use super::service::{priority_for, TilePixels, COMPLETION_BATCH, COMPLETION_FLUSH};
use super::sidecars::{decode, encode, SidecarStore, StoredTile};
use super::{ThumbService, TileIdentity};

/// How long a fetch waits for the owner's next answer. The owner bakes what
/// it lacks before answering, and a large video can use most of its decoder's
/// 30 second limit.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// Asks per request, which bounds how much baking one request can queue on
/// the owner.
const MAX_ASKS: usize = 64;

/// One tile this device wants from an owner's side, and where it goes.
struct Pending {
	cache_id: Uuid,
	uuid: Uuid,
	path: PathBuf,
	/// The version the hot cache holds, zero for none.
	held: u64,
	/// This device's copy of the owner's sidecars for the file's source.
	copy: Option<Arc<SidecarStore>>,
	/// The cache holds nothing and the copy holds a tile to restore.
	restore: bool,
	/// Not asked of the owner within the re-ask window.
	ask: bool,
}

/// A file a replica lists, and where its tile lives here.
struct RemoteFile {
	device_id: Uuid,
	source_id: Uuid,
	cache_id: Uuid,
	uuid: Uuid,
}

/// Identities for files in paired devices' replicas, each `(device slug,
/// path)`. `None` where no replica lists the path as a file.
///
/// Tiles are restored from this device's copies of the owners' sidecars, and
/// anything missing or not confirmed recently is asked of the owner, both in
/// the background; see the module docs.
pub async fn request(
	context: &Arc<CoreContext>,
	paths: &[(String, PathBuf)],
) -> Vec<Option<TileIdentity>> {
	let mut resolved = Vec::with_capacity(paths.len());
	for (device_slug, path) in paths {
		resolved.push(resolve(context, device_slug, path).await);
	}

	let keys: Vec<(Uuid, Uuid)> = resolved
		.iter()
		.flatten()
		.map(|file| (file.cache_id, file.uuid))
		.collect();
	let thumbs = context.thumbs.clone();
	let mut held = tokio::task::spawn_blocking(move || thumbs.held_versions(&keys))
		.await
		.unwrap_or_default()
		.into_iter();

	let mut work: HashMap<Uuid, Vec<Pending>> = HashMap::new();
	let mut identities = Vec::with_capacity(paths.len());
	for (resolved, (_, path)) in resolved.into_iter().zip(paths) {
		let Some(file) = resolved else {
			identities.push(None);
			continue;
		};
		let held = held.next().flatten();
		let copy = context
			.thumbs
			.sidecar_store(&peer::sidecar_copy_file(
				context,
				file.device_id,
				file.source_id,
			))
			.await;
		let restore = held.is_none()
			&& copy
				.as_ref()
				.is_some_and(|copy| copy.version_of(file.uuid).is_some());
		let ask = context.thumbs.claim_remote(file.cache_id, file.uuid);
		if restore || ask {
			work.entry(file.device_id).or_default().push(Pending {
				cache_id: file.cache_id,
				uuid: file.uuid,
				path: path.clone(),
				held: held.unwrap_or(0),
				copy,
				restore,
				ask,
			});
		}
		identities.push(Some(TileIdentity {
			source_id: file.cache_id,
			uuid: file.uuid,
			version: held.unwrap_or(0),
		}));
	}

	for (device_id, pending) in work {
		tokio::spawn(fetch(context.clone(), device_id, pending));
	}
	identities
}

/// The replica that lists `path` as a file, the cache its tile belongs in,
/// and the uuid the replica knows it by.
async fn resolve(
	context: &Arc<CoreContext>,
	device_slug: &str,
	path: &PathBuf,
) -> Option<RemoteFile> {
	let share = peer::share_for(context, device_slug, path).await?;
	let uuid = {
		let mut index = share.index.write().await;
		if index.get_entry_ref(path)?.kind != EntryKind::File {
			return None;
		}
		index.get_or_assign_uuid(path)
	};
	Some(RemoteFile {
		device_id: share.device_id,
		source_id: share.info.id,
		// Keyed by the owner's volume, as local tiles are keyed by the drive.
		// A share whose owner could not name its volume is keyed by the
		// source.
		cache_id: share.info.volume_uuid.unwrap_or(share.info.id),
		uuid,
	})
}

/// Restore what this device's copies hold, then ask the owner for the rest,
/// a request at a time, storing each tile as it lands.
async fn fetch(context: Arc<CoreContext>, device_id: Uuid, pending: Vec<Pending>) {
	let asks = restore_from_copies(&context, pending).await;
	for chunk in chunks(asks) {
		let mut unanswered = vec![true; chunk.len()];
		if let Err(error) = fetch_chunk(&context, device_id, &chunk, &mut unanswered).await {
			debug!(%device_id, "thumbnail fetch failed: {error:#}");
			let retry: Vec<(Uuid, Uuid)> = chunk
				.iter()
				.zip(&unanswered)
				.filter(|(_, unanswered)| **unanswered)
				.map(|(ask, _)| (ask.cache_id, ask.uuid))
				.collect();
			context.thumbs.release_remote(&retry);
		}
	}
}

/// Put tiles back in the hot cache from this device's copies of the owners'
/// sidecars, which needs no network and works with the owner asleep. What is
/// restored goes to the owner as held, so an unchanged tile comes back as
/// `Current`. Returns what is left to ask.
async fn restore_from_copies(
	context: &Arc<CoreContext>,
	mut pending: Vec<Pending>,
) -> Vec<Pending> {
	let mut stored = Vec::new();
	for (index, want) in pending.iter().enumerate() {
		let Some(copy) = want.copy.as_ref().filter(|_| want.restore) else {
			continue;
		};
		if let Ok(Some(tile)) = copy.read(want.uuid).await {
			stored.push((index, want.cache_id, want.uuid, tile));
		}
	}
	if !stored.is_empty() {
		let thumbs = context.thumbs.clone();
		let restored = tokio::task::spawn_blocking(move || {
			let mut tiles = Vec::new();
			let mut restored = Vec::new();
			for (index, cache_id, uuid, tile) in stored {
				if let Some(bgra) = decode(&tile.webp, tile.frame) {
					restored.push((index, tile.version));
					tiles.push((
						cache_id,
						uuid,
						TilePixels {
							version: tile.version,
							frame: tile.frame,
							bgra,
						},
					));
				}
			}
			thumbs.write_tiles(tiles);
			restored
		})
		.await
		.unwrap_or_default();
		for (index, version) in restored {
			pending[index].held = version;
		}
	}
	pending.into_iter().filter(|want| want.ask).collect()
}

/// One request's worth of asks. Landed tiles are stored in batches on the
/// drain's schedule, and once more on the way out whatever happened, so an
/// owner that fails halfway still leaves the tiles it sent. Each is also
/// kept in this device's copy of the owner's sidecars, as the WebP it came
/// in as.
async fn fetch_chunk(
	context: &Arc<CoreContext>,
	device_id: Uuid,
	chunk: &[Pending],
	unanswered: &mut [bool],
) -> anyhow::Result<()> {
	let asks = chunk
		.iter()
		.map(|ask| TileAsk {
			path: ask.path.clone(),
			held: ask.held,
		})
		.collect();
	let answers = peer::fetch_tiles(context, device_id, asks).await?;
	futures::pin_mut!(answers);

	let mut landed = Landed::default();
	let result: anyhow::Result<()> = async {
		let mut oldest: Option<Instant> = None;
		loop {
			let wait = oldest.map_or(ANSWER_TIMEOUT, |first| {
				COMPLETION_FLUSH.saturating_sub(first.elapsed())
			});
			let next = match tokio::time::timeout(wait, answers.next()).await {
				Ok(next) => next,
				Err(_) if oldest.is_some() => {
					landed.store(context).await;
					oldest = None;
					continue;
				}
				Err(_) => anyhow::bail!("no answer within {ANSWER_TIMEOUT:?}"),
			};
			let Some(next) = next else {
				return Ok(());
			};
			let (index, answer, bytes) = next?;
			let Some(ask) = chunk.get(index) else {
				anyhow::bail!("answer to ask {index} of {}", chunk.len());
			};
			unanswered[index] = false;

			let TileAnswer::Tile {
				version,
				content_width,
				content_height,
				source_width,
				source_height,
				..
			} = answer
			else {
				continue;
			};
			let frame = Frame {
				content_width,
				content_height,
				source_width,
				source_height,
			};
			let Ok((Some(bgra), webp)) =
				tokio::task::spawn_blocking(move || (decode(&bytes, frame), bytes)).await
			else {
				debug!(path = %ask.path.display(), "undecodable tile from {device_id}");
				continue;
			};
			landed.tiles.push((
				ask.cache_id,
				ask.uuid,
				TilePixels {
					version,
					frame,
					bgra,
				},
			));
			if let Some(copy) = &ask.copy {
				landed.kept.push((
					copy.clone(),
					ask.uuid,
					StoredTile {
						version,
						frame,
						webp,
					},
				));
			}
			oldest.get_or_insert_with(Instant::now);
			if landed.tiles.len() >= COMPLETION_BATCH {
				landed.store(context).await;
				oldest = None;
			}
		}
	}
	.await;
	landed.store(context).await;
	result
}

/// Tiles fetched and not yet stored: pixels for the hot cache, and the WebP
/// they arrived as for this device's copies of the owner's sidecars.
#[derive(Default)]
struct Landed {
	tiles: Vec<(Uuid, Uuid, TilePixels)>,
	kept: Vec<(Arc<SidecarStore>, Uuid, StoredTile)>,
}

impl Landed {
	async fn store(&mut self, context: &Arc<CoreContext>) {
		if !self.tiles.is_empty() {
			let tiles = std::mem::take(&mut self.tiles);
			let thumbs = context.thumbs.clone();
			let _ = tokio::task::spawn_blocking(move || thumbs.write_tiles(tiles)).await;
		}
		let mut by_copy: Vec<(Arc<SidecarStore>, Vec<(Uuid, StoredTile)>)> = Vec::new();
		for (copy, uuid, tile) in self.kept.drain(..) {
			match by_copy
				.iter_mut()
				.find(|(held, _)| Arc::ptr_eq(held, &copy))
			{
				Some((_, tiles)) => tiles.push((uuid, tile)),
				None => by_copy.push((copy, vec![(uuid, tile)])),
			}
		}
		for (copy, tiles) in by_copy {
			if let Err(error) = copy.keep(tiles).await {
				debug!("keeping fetched tiles: {error:#}");
			}
		}
	}
}

/// Split asks into requests that each fit the protocol's frame limit with
/// room to spare, since a path can be as long as its filesystem allows.
fn chunks(pending: Vec<Pending>) -> Vec<Vec<Pending>> {
	let budget = MAX_REQUEST_FRAME as usize / 2;
	let mut chunks = Vec::new();
	let mut current: Vec<Pending> = Vec::new();
	let mut size = 0;
	for ask in pending {
		// The path, plus the held version and msgpack's framing around both.
		let cost = ask.path.as_os_str().len() + 16;
		if !current.is_empty() && (size + cost > budget || current.len() == MAX_ASKS) {
			chunks.push(std::mem::take(&mut current));
			size = 0;
		}
		size += cost;
		current.push(ask);
	}
	if !current.is_empty() {
		chunks.push(current);
	}
	chunks
}

/// What to send a peer for `ask`, the `rank`-th of `total` in one request:
/// nothing when it already holds this device's version, the tile when this
/// device has one or can bake one, and `Missing` otherwise. A tile the
/// sidecars hold at this version goes out as the WebP it is stored as. Files
/// are baked in request order, as a local viewport's are.
pub async fn answer(
	thumbs: &Arc<ThumbService>,
	ask: TileAsk,
	rank: usize,
	total: usize,
) -> (TileAnswer, Vec<u8>) {
	let Some(identity) = thumbs.thumbnail_identity(&ask.path).await else {
		return (TileAnswer::Missing, Vec::new());
	};
	if identity.version == ask.held {
		return (TileAnswer::Current, Vec::new());
	}
	if let Some(stored) = thumbs.stored_tile(&ask.path, identity).await {
		return tile_answer(stored);
	}
	let Some(tile) = thumbs
		.fresh_tile(&ask.path, identity, priority_for(rank, total))
		.await
	else {
		return (TileAnswer::Missing, Vec::new());
	};
	let encoded = tokio::task::spawn_blocking(move || {
		Some(StoredTile {
			version: tile.version,
			webp: encode(&tile)?,
			frame: tile.frame,
		})
	})
	.await;
	match encoded {
		Ok(Some(stored)) => tile_answer(stored),
		_ => (TileAnswer::Missing, Vec::new()),
	}
}

fn tile_answer(stored: StoredTile) -> (TileAnswer, Vec<u8>) {
	(
		TileAnswer::Tile {
			version: stored.version,
			content_width: stored.frame.content_width,
			content_height: stored.frame.content_height,
			source_width: stored.frame.source_width,
			source_height: stored.frame.source_height,
			len: stored.webp.len() as u64,
		},
		stored.webp,
	)
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::Ordering;

	use super::super::service::tests::{fixture, source_fixture};
	use super::super::ThumbnailGenerationMode;
	use super::*;

	#[tokio::test]
	async fn an_owner_sends_a_tile_then_confirms_it_without_rebaking() {
		let (_temp, service, path, calls, _) = fixture().await;
		let ask = |held| TileAsk {
			path: path.clone(),
			held,
		};

		let (sent, bytes) = answer(&service, ask(0), 0, 1).await;
		let TileAnswer::Tile {
			version,
			content_width,
			content_height,
			source_width,
			source_height,
			len,
		} = sent
		else {
			panic!("expected a tile, got {sent:?}");
		};
		assert_eq!(len, bytes.len() as u64);
		let frame = Frame {
			content_width,
			content_height,
			source_width,
			source_height,
		};
		assert!(decode(&bytes, frame).is_some(), "the tile decodes");
		assert_eq!(calls.load(Ordering::SeqCst), 1);

		assert_eq!(
			answer(&service, ask(version), 0, 1).await,
			(TileAnswer::Current, Vec::new()),
			"a peer holding this version is told so"
		);
		let (resent, _) = answer(&service, ask(version.wrapping_add(1)), 0, 1).await;
		assert!(
			matches!(resent, TileAnswer::Tile { .. }),
			"an old version is replaced"
		);
		assert_eq!(
			calls.load(Ordering::SeqCst),
			1,
			"from the cache, not a rebake"
		);
	}

	#[tokio::test]
	async fn a_tile_with_a_sidecar_goes_out_as_stored() {
		let (_temp, service, path, _, _) = source_fixture().await;
		service
			.generate_one(&path, ThumbnailGenerationMode::Missing)
			.await;
		let identity = service.thumbnail_identity(&path).await.unwrap();
		let stored = tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				if let Some(stored) = service.stored_tile(&path, identity).await {
					return stored;
				}
				tokio::time::sleep(Duration::from_millis(20)).await;
			}
		})
		.await
		.expect("the sidecar lands");

		let (sent, bytes) = answer(
			&service,
			TileAsk {
				path: path.clone(),
				held: 0,
			},
			0,
			1,
		)
		.await;
		assert!(matches!(sent, TileAnswer::Tile { .. }));
		assert_eq!(
			bytes, stored.webp,
			"sent without decoding or encoding again"
		);
	}

	#[tokio::test]
	async fn an_owner_without_the_file_answers_missing() {
		let (temp, service, _, _, _) = fixture().await;
		let ask = TileAsk {
			path: temp.path().join("never-indexed.png"),
			held: 0,
		};
		assert_eq!(
			answer(&service, ask, 0, 1).await,
			(TileAnswer::Missing, Vec::new())
		);
	}

	#[test]
	fn requests_stay_inside_the_frame_limit() {
		use crate::service::network::protocol::byterange::ByteRangeRequest;

		let ask = |length: usize| Pending {
			cache_id: Uuid::nil(),
			uuid: Uuid::nil(),
			path: PathBuf::from("/".repeat(length)),
			held: 0,
			copy: None,
			restore: false,
			ask: true,
		};

		let short = chunks((0..200).map(|_| ask(100)).collect());
		assert_eq!(
			short.iter().map(Vec::len).collect::<Vec<_>>(),
			[MAX_ASKS, MAX_ASKS, MAX_ASKS, 200 - 3 * MAX_ASKS]
		);

		let long = chunks((0..40).map(|_| ask(4096)).collect());
		for chunk in &long {
			let request = rmp_serde::to_vec(&ByteRangeRequest::FetchTiles {
				tiles: chunk
					.iter()
					.map(|ask| TileAsk {
						path: ask.path.clone(),
						held: u64::MAX,
					})
					.collect(),
			})
			.unwrap();
			assert!(request.len() <= MAX_REQUEST_FRAME as usize);
		}
		assert_eq!(long.iter().map(Vec::len).sum::<usize>(), 40);
	}
}
