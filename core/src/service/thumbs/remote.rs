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
use super::{ThumbService, TileIdentity, TILE};

const WEBP_QUALITY: f32 = 80.0;

/// How long a fetch waits for the owner's next answer. The owner bakes what
/// it lacks before answering, and a large video can use most of its decoder's
/// 30 second limit.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// Asks per request, which bounds how much baking one request can queue on
/// the owner.
const MAX_ASKS: usize = 64;

/// One tile to ask an owner for, and where it goes when it lands.
struct Pending {
	cache_id: Uuid,
	uuid: Uuid,
	path: PathBuf,
	held: u64,
}

/// Identities for files in paired devices' replicas, each `(device slug,
/// path)`. `None` where no replica lists the path as a file.
///
/// Tiles this device does not hold, or has not confirmed with the owner
/// recently, are fetched in the background; see the module docs.
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
		.map(|&(_, cache_id, uuid)| (cache_id, uuid))
		.collect();
	let thumbs = context.thumbs.clone();
	let mut held = tokio::task::spawn_blocking(move || thumbs.held_versions(&keys))
		.await
		.unwrap_or_default()
		.into_iter();

	let mut asks: HashMap<Uuid, Vec<Pending>> = HashMap::new();
	let identities = resolved
		.into_iter()
		.zip(paths)
		.map(|(resolved, (_, path))| {
			let (device_id, cache_id, uuid) = resolved?;
			let held = held.next().flatten().unwrap_or(0);
			if context.thumbs.claim_remote(cache_id, uuid) {
				asks.entry(device_id).or_default().push(Pending {
					cache_id,
					uuid,
					path: path.clone(),
					held,
				});
			}
			Some(TileIdentity {
				source_id: cache_id,
				uuid,
				version: held,
			})
		})
		.collect();

	for (device_id, pending) in asks {
		tokio::spawn(fetch(context.clone(), device_id, pending));
	}
	identities
}

/// The device whose replica lists `path` as a file, the cache its tile
/// belongs in, and the uuid the replica knows it by.
async fn resolve(
	context: &Arc<CoreContext>,
	device_slug: &str,
	path: &PathBuf,
) -> Option<(Uuid, Uuid, Uuid)> {
	let share = peer::share_for(context, device_slug, path).await?;
	let uuid = {
		let mut index = share.index.write().await;
		if index.get_entry_ref(path)?.kind != EntryKind::File {
			return None;
		}
		index.get_or_assign_uuid(path)
	};
	// Keyed by the owner's volume, as local tiles are keyed by the drive. A
	// share whose owner could not name its volume is keyed by the source.
	let cache_id = share.info.volume_uuid.unwrap_or(share.info.id);
	Some((share.device_id, cache_id, uuid))
}

/// Ask one owner for tiles, a request at a time, storing each as it lands.
async fn fetch(context: Arc<CoreContext>, device_id: Uuid, pending: Vec<Pending>) {
	for chunk in chunks(pending) {
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

/// One request's worth of asks. Landed tiles are stored in batches on the
/// drain's schedule, and once more on the way out whatever happened, so an
/// owner that fails halfway still leaves the tiles it sent.
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

	let mut landed = Vec::new();
	let result: anyhow::Result<()> = async {
		let mut oldest: Option<Instant> = None;
		loop {
			let wait = oldest.map_or(ANSWER_TIMEOUT, |first| {
				COMPLETION_FLUSH.saturating_sub(first.elapsed())
			});
			let next = match tokio::time::timeout(wait, answers.next()).await {
				Ok(next) => next,
				Err(_) if oldest.is_some() => {
					store(context, &mut landed).await;
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
			let decoded = tokio::task::spawn_blocking(move || decode(&bytes, frame))
				.await
				.ok()
				.flatten();
			let Some(bgra) = decoded else {
				debug!(path = %ask.path.display(), "undecodable tile from {device_id}");
				continue;
			};
			landed.push((
				ask.cache_id,
				ask.uuid,
				TilePixels {
					version,
					frame,
					bgra,
				},
			));
			oldest.get_or_insert_with(Instant::now);
			if landed.len() >= COMPLETION_BATCH {
				store(context, &mut landed).await;
				oldest = None;
			}
		}
	}
	.await;
	store(context, &mut landed).await;
	result
}

async fn store(context: &Arc<CoreContext>, landed: &mut Vec<(Uuid, Uuid, TilePixels)>) {
	if landed.is_empty() {
		return;
	}
	let tiles = std::mem::take(landed);
	let thumbs = context.thumbs.clone();
	let _ = tokio::task::spawn_blocking(move || thumbs.store_fetched(tiles)).await;
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
/// device has one or can bake one, and `Missing` otherwise. Files are baked in
/// request order, as a local viewport's are.
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
	let Some(tile) = thumbs
		.fresh_tile(&ask.path, identity, priority_for(rank, total))
		.await
	else {
		return (TileAnswer::Missing, Vec::new());
	};
	let frame = tile.frame;
	let version = tile.version;
	let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || encode(&tile)).await else {
		return (TileAnswer::Missing, Vec::new());
	};
	(
		TileAnswer::Tile {
			version,
			content_width: frame.content_width,
			content_height: frame.content_height,
			source_width: frame.source_width,
			source_height: frame.source_height,
			len: bytes.len() as u64,
		},
		bytes,
	)
}

/// A tile as lossy WebP.
fn encode(tile: &TilePixels) -> Option<Vec<u8>> {
	let Frame {
		content_width: width,
		content_height: height,
		..
	} = tile.frame;
	if width == 0 || height == 0 || tile.bgra.len() != tile.frame.len() {
		return None;
	}
	// Tiles are stored BGRA8, which is what a GPU atlas wants; WebP takes RGBA.
	let mut rgba = tile.bgra.clone();
	for pixel in rgba.chunks_exact_mut(4) {
		pixel.swap(0, 2);
	}
	Some(
		webp::Encoder::from_rgba(&rgba, width, height)
			.encode(WEBP_QUALITY)
			.to_vec(),
	)
}

/// A WebP tile back to BGRA8, provided it has the size its header declared
/// and fits a slot.
fn decode(bytes: &[u8], frame: Frame) -> Option<Vec<u8>> {
	if frame.content_width > TILE || frame.content_height > TILE {
		return None;
	}
	let image = webp::Decoder::new(bytes).decode()?;
	if image.width() != frame.content_width || image.height() != frame.content_height {
		return None;
	}
	// The decoder drops the alpha channel of an opaque image.
	let channels = if image.is_alpha() { 4 } else { 3 };
	let mut bgra = Vec::with_capacity(frame.len());
	for pixel in image.chunks_exact(channels) {
		let alpha = if channels == 4 { pixel[3] } else { u8::MAX };
		bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], alpha]);
	}
	(bgra.len() == frame.len()).then_some(bgra)
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::Ordering;

	use super::super::service::tests::fixture;
	use super::*;

	fn frame(width: u32, height: u32) -> Frame {
		Frame {
			content_width: width,
			content_height: height,
			source_width: width * 10,
			source_height: height * 10,
		}
	}

	/// A gradient, so a lossy round trip has something to get wrong.
	fn gradient(width: u32, height: u32, alpha: impl Fn(u32, u32) -> u8) -> TilePixels {
		let mut bgra = Vec::new();
		for y in 0..height {
			for x in 0..width {
				bgra.extend_from_slice(&[
					(x * 255 / width) as u8,
					(y * 255 / height) as u8,
					128,
					alpha(x, y),
				]);
			}
		}
		TilePixels {
			version: 7,
			frame: frame(width, height),
			bgra,
		}
	}

	fn max_error(a: &[u8], b: &[u8]) -> u8 {
		a.iter()
			.zip(b)
			.map(|(a, b)| a.abs_diff(*b))
			.max()
			.unwrap_or(0)
	}

	#[test]
	fn an_opaque_tile_survives_the_wire() {
		let tile = gradient(384, 216, |_, _| 255);
		let bytes = encode(&tile).expect("encodes");
		assert!(
			bytes.len() < tile.bgra.len() / 10,
			"{} bytes for {} of pixels",
			bytes.len(),
			tile.bgra.len()
		);

		let decoded = decode(&bytes, tile.frame).expect("decodes");
		assert_eq!(decoded.len(), tile.bgra.len());
		assert!(decoded.chunks_exact(4).all(|pixel| pixel[3] == 255));
		assert!(
			max_error(&decoded, &tile.bgra) < 24,
			"channels stay in order"
		);
	}

	#[test]
	fn transparency_survives_the_wire() {
		let tile = gradient(64, 64, |x, _| if x < 32 { 0 } else { 255 });
		let decoded = decode(&encode(&tile).expect("encodes"), tile.frame).expect("decodes");
		assert_eq!(decoded[3], 0, "the left edge stays transparent");
		assert_eq!(
			decoded[decoded.len() - 1],
			255,
			"the right edge stays opaque"
		);
	}

	#[test]
	fn a_tile_that_disagrees_with_its_header_is_refused() {
		let tile = gradient(64, 48, |_, _| 255);
		let bytes = encode(&tile).expect("encodes");
		assert!(decode(&bytes, frame(48, 64)).is_none(), "wrong size");
		assert!(
			decode(&bytes, frame(TILE + 1, 48)).is_none(),
			"wider than a slot"
		);
		assert!(decode(b"not a webp", tile.frame).is_none(), "not an image");
	}

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
