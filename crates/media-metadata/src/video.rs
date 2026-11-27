//! # Video container headers
//!
//! Reads a video's duration, frame size and codec from the container header
//! alone, without `FFmpeg` and without decoding a frame, so every build can
//! record them while it indexes. Two families cover most footage: ISO base
//! media (MP4, MOV, M4V, 3GP), read from its `moov` box wherever the file
//! puts it, and Matroska (MKV, `WebM`), read from its `Info` and `Tracks`
//! elements ahead of the first cluster. Anything else, or a header that does
//! not say, answers `None` and the caller falls back to what footage work
//! measures later.
//!
//! ## Example
//! ```rust,no_run
//! use sd_media_metadata::video::read_header;
//!
//! if let Some(header) = read_header(std::path::Path::new("clip.mp4"))? {
//!     let minutes = header.duration_seconds.unwrap_or_default() / 60.0;
//!     assert!(minutes >= 0.0);
//! }
//! # Ok::<(), std::io::Error>(())
//! ```

use std::{
	fs::File,
	io::{self, BufReader, Read, Seek, SeekFrom},
	path::Path,
};

/// The largest `moov` box or Matroska header element read into memory. A
/// long recording's sample tables run to a few megabytes; past this the
/// file is not something a header read should load.
const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;

/// What a container header says about a video.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoHeader {
	pub duration_seconds: Option<f64>,
	pub width: Option<u32>,
	pub height: Option<u32>,
	/// The codec as `FFmpeg` names it (`h264`, `hevc`, `vp9`), so a value
	/// written here matches one a probe wrote.
	pub codec: Option<String>,
}

impl VideoHeader {
	fn is_empty(&self) -> bool {
		*self == Self::default()
	}
}

/// Read the header of the video at `path`.
///
/// Returns `Ok(None)` for a container this reader does not know or a header
/// that names nothing, and an error only when the file cannot be read.
pub fn read_header(path: &Path) -> io::Result<Option<VideoHeader>> {
	let mut file = BufReader::new(File::open(path)?);
	let len = file.get_ref().metadata()?.len();
	let mut magic = [0u8; 8];
	if len < 8 {
		return Ok(None);
	}
	file.read_exact(&mut magic)?;
	file.seek(SeekFrom::Start(0))?;
	let header = if magic[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
		matroska::read(&mut file, len)?
	} else if matches!(
		&magic[4..8],
		b"ftyp" | b"moov" | b"mdat" | b"wide" | b"free" | b"skip"
	) {
		iso::read(&mut file, len)?
	} else {
		None
	};
	Ok(header.filter(|header| !header.is_empty()))
}

fn read_body<R: Read>(reader: &mut R, size: u64) -> io::Result<Option<Vec<u8>>> {
	if size > MAX_HEADER_BYTES {
		return Ok(None);
	}
	let mut body = vec![0; usize::try_from(size).map_err(|_| io::ErrorKind::InvalidData)?];
	reader.read_exact(&mut body)?;
	Ok(Some(body))
}

fn be(bytes: &[u8]) -> u64 {
	bytes
		.iter()
		.fold(0, |value, &byte| (value << 8) | u64::from(byte))
}

/// ISO base media: boxes of a 32-bit size (or 1 and a 64-bit size, or 0 for
/// "to the end") and a four-character type.
mod iso {
	use super::{be, read_body, Read, Seek, SeekFrom, VideoHeader};
	use std::io;

	pub fn read<R: Read + Seek>(file: &mut R, len: u64) -> io::Result<Option<VideoHeader>> {
		let mut at = 0;
		while at + 8 <= len {
			file.seek(SeekFrom::Start(at))?;
			let mut head = [0u8; 16];
			file.read_exact(&mut head[..8])?;
			let (mut size, mut header) = (be(&head[..4]), 8);
			if size == 1 {
				file.read_exact(&mut head[8..16])?;
				size = be(&head[8..16]);
				header = 16;
			} else if size == 0 {
				size = len - at;
			}
			// A box that claims more than the file holds is corrupt, and
			// following it could wrap the cursor back over the same boxes.
			if size < header || size > len - at {
				return Ok(None);
			}
			if &head[4..8] == b"moov" {
				return Ok(read_body(file, size - header)?.map(|moov| movie(&moov)));
			}
			at += size;
		}
		Ok(None)
	}

	/// The children of a box body, as (type, body) pairs. Stops at the first
	/// child whose size does not fit, which is as far as a truncated header
	/// can be trusted.
	fn children(body: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
		let mut rest = body;
		std::iter::from_fn(move || {
			if rest.len() < 8 {
				return None;
			}
			let (size, header) = match be(&rest[..4]) {
				1 if rest.len() >= 16 => (be(&rest[8..16]), 16),
				0 => (rest.len() as u64, 8),
				size => (size, 8),
			};
			let size = usize::try_from(size).ok()?;
			if size < header || size > rest.len() {
				return None;
			}
			let child = (&rest[4..8], &rest[header..size]);
			rest = &rest[size..];
			Some(child)
		})
	}

	fn child<'a>(body: &'a [u8], kind: &[u8]) -> Option<&'a [u8]> {
		children(body)
			.find(|(child, _)| *child == kind)
			.map(|(_, body)| body)
	}

	fn movie(moov: &[u8]) -> VideoHeader {
		let mut header = VideoHeader {
			duration_seconds: child(moov, b"mvhd").and_then(duration),
			..VideoHeader::default()
		};
		if let Some(track) = children(moov)
			.filter(|(kind, _)| *kind == b"trak")
			.find_map(|(_, trak)| video_track(trak))
		{
			header.width = track.width;
			header.height = track.height;
			header.codec = track.codec;
			header.duration_seconds = header.duration_seconds.or(track.duration_seconds);
		}
		header
	}

	/// `mvhd` and `mdhd` share the layout this reads: a version, then
	/// creation and modification times, a timescale and a duration, each
	/// time 32 bits wide in version 0 and 64 in version 1.
	fn duration(body: &[u8]) -> Option<f64> {
		let wide = *body.first()? == 1;
		let (scale_at, duration_at, duration_len) = if wide { (20, 24, 8) } else { (12, 16, 4) };
		let timescale = be(body.get(scale_at..scale_at + 4)?);
		let duration = be(body.get(duration_at..duration_at + duration_len)?);
		let unknown = if wide { u64::MAX } else { u64::from(u32::MAX) };
		if timescale == 0 || duration == 0 || duration == unknown {
			return None;
		}
		#[allow(clippy::cast_precision_loss)]
		Some(duration as f64 / timescale as f64)
	}

	fn video_track(trak: &[u8]) -> Option<VideoHeader> {
		let mdia = child(trak, b"mdia")?;
		if child(mdia, b"hdlr")?.get(8..12)? != b"vide" {
			return None;
		}
		let entry = child(mdia, b"minf")
			.and_then(|minf| child(minf, b"stbl"))
			.and_then(|stbl| child(stbl, b"stsd"))
			.and_then(|stsd| stsd.get(8..));
		let format = entry.and_then(|entry| entry.get(4..8));
		// The visual sample entry's coded size, after the 8-byte entry
		// header, 8 reserved bytes and 16 of predefined fields.
		let coded = entry.and_then(|entry| Some((be(entry.get(32..34)?), be(entry.get(34..36)?))));
		// tkhd's display size is 16.16 fixed point at the end of its body,
		// which is 84 bytes in version 0 and 96 in version 1.
		let display = child(trak, b"tkhd").and_then(|tkhd| {
			let at = if *tkhd.first()? == 1 { 88 } else { 76 };
			Some((
				be(tkhd.get(at..at + 4)?) >> 16,
				be(tkhd.get(at + 4..at + 8)?) >> 16,
			))
		});
		let (width, height) = display
			.filter(|&(w, h)| w > 0 && h > 0)
			.or_else(|| coded.filter(|&(w, h)| w > 0 && h > 0))
			.map_or((None, None), |(w, h)| {
				(u32::try_from(w).ok(), u32::try_from(h).ok())
			});
		Some(VideoHeader {
			duration_seconds: child(mdia, b"mdhd").and_then(duration),
			width,
			height,
			codec: format.map(codec),
		})
	}

	fn codec(format: &[u8]) -> String {
		match format {
			b"avc1" | b"avc3" => "h264",
			b"hvc1" | b"hev1" => "hevc",
			b"av01" => "av1",
			b"vp08" => "vp8",
			b"vp09" => "vp9",
			b"mp4v" => "mpeg4",
			b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "prores",
			b"jpeg" | b"mjpa" | b"mjpb" => "mjpeg",
			other => return String::from_utf8_lossy(other).trim().to_lowercase(),
		}
		.to_string()
	}
}

/// Matroska: EBML elements of a variable-length id and size. The segment's
/// `Info` and `Tracks` come before its clusters in every muxer this has
/// met, so the read stops at the first cluster.
mod matroska {
	use super::{be, read_body, Read, Seek, SeekFrom, VideoHeader};
	use std::io;

	const SEGMENT: u64 = 0x1853_8067;
	const INFO: u64 = 0x1549_A966;
	const TRACKS: u64 = 0x1654_AE6B;
	const CLUSTER: u64 = 0x1F43_B675;
	const TIMESTAMP_SCALE: u64 = 0x2A_D7B1;
	const DURATION: u64 = 0x4489;
	const TRACK_ENTRY: u64 = 0xAE;
	const TRACK_TYPE: u64 = 0x83;
	const CODEC_ID: u64 = 0x86;
	const VIDEO: u64 = 0xE0;
	const PIXEL_WIDTH: u64 = 0xB0;
	const PIXEL_HEIGHT: u64 = 0xBA;

	/// A variable-length integer: the leading zero bits of the first byte
	/// give its length. An id keeps its marker bit, a size drops it, and a
	/// size of all ones means "unknown".
	fn vint(bytes: &[u8], keep_marker: bool) -> Option<(Option<u64>, usize)> {
		let first = *bytes.first()?;
		let width = first.leading_zeros() + 1;
		let len = width as usize;
		if len > 8 || bytes.len() < len {
			return None;
		}
		let raw = be(&bytes[..len]);
		if keep_marker {
			return Some((Some(raw), len));
		}
		let bits = 7 * width;
		let value = raw & ((1u64 << bits) - 1);
		Some(((value != (1u64 << bits) - 1).then_some(value), len))
	}

	fn element<R: Read>(file: &mut R) -> io::Result<Option<(u64, Option<u64>, u64)>> {
		let mut head = [0u8; 12];
		file.read_exact(&mut head[..1])?;
		let id_len = head[0].leading_zeros() as usize + 1;
		if id_len > 4 {
			return Ok(None);
		}
		file.read_exact(&mut head[1..id_len])?;
		file.read_exact(&mut head[id_len..=id_len])?;
		let size_len = head[id_len].leading_zeros() as usize + 1;
		if size_len > 8 {
			return Ok(None);
		}
		file.read_exact(&mut head[id_len + 1..id_len + size_len])?;
		let Some((Some(id), _)) = vint(&head[..id_len], true) else {
			return Ok(None);
		};
		let Some((size, _)) = vint(&head[id_len..id_len + size_len], false) else {
			return Ok(None);
		};
		Ok(Some((id, size, (id_len + size_len) as u64)))
	}

	fn children(body: &[u8]) -> impl Iterator<Item = (u64, &[u8])> {
		let mut rest = body;
		std::iter::from_fn(move || {
			let (Some(id), id_len) = vint(rest, true)? else {
				return None;
			};
			let (size, size_len) = vint(rest.get(id_len..)?, false)?;
			let start = id_len + size_len;
			let end = start + usize::try_from(size?).ok()?;
			let child = (id, rest.get(start..end)?);
			rest = &rest[end..];
			Some(child)
		})
	}

	fn float(bytes: &[u8]) -> Option<f64> {
		match bytes.len() {
			4 => Some(f64::from(f32::from_be_bytes(bytes.try_into().ok()?))),
			8 => Some(f64::from_be_bytes(bytes.try_into().ok()?)),
			_ => None,
		}
	}

	pub fn read<R: Read + Seek>(file: &mut R, len: u64) -> io::Result<Option<VideoHeader>> {
		let mut at = 0;
		// Find the segment past the EBML header.
		let segment_start = loop {
			if at >= len {
				return Ok(None);
			}
			file.seek(SeekFrom::Start(at))?;
			let Some((id, size, head)) = element(file)? else {
				return Ok(None);
			};
			if id == SEGMENT {
				break at + head;
			}
			let Some(next) = size.and_then(|size| at.checked_add(head)?.checked_add(size)) else {
				return Ok(None);
			};
			at = next;
		};

		let mut header = VideoHeader::default();
		let (mut info, mut tracks) = (false, false);
		at = segment_start;
		while at < len && !(info && tracks) {
			file.seek(SeekFrom::Start(at))?;
			let Some((id, Some(size), head)) = element(file)? else {
				break;
			};
			match id {
				CLUSTER => break,
				INFO => {
					info = true;
					if let Some(body) = read_body(file, size)? {
						let mut scale = 1_000_000.0;
						let mut duration = None;
						for (child, value) in children(&body) {
							match child {
								#[allow(clippy::cast_precision_loss)]
								TIMESTAMP_SCALE => scale = be(value) as f64,
								DURATION => duration = float(value),
								_ => {}
							}
						}
						header.duration_seconds = duration
							.map(|duration| duration * scale / 1e9)
							.filter(|seconds| seconds.is_finite() && *seconds > 0.0);
					}
				}
				TRACKS => {
					tracks = true;
					if let Some(body) = read_body(file, size)? {
						if let Some(track) = children(&body)
							.filter(|(child, _)| *child == TRACK_ENTRY)
							.find_map(|(_, entry)| video_track(entry))
						{
							header.width = track.width;
							header.height = track.height;
							header.codec = track.codec;
						}
					}
				}
				_ => {}
			}
			let Some(next) = at.checked_add(head).and_then(|at| at.checked_add(size)) else {
				break;
			};
			at = next;
		}
		Ok(Some(header))
	}

	fn video_track(entry: &[u8]) -> Option<VideoHeader> {
		let mut is_video = false;
		let mut header = VideoHeader::default();
		for (id, value) in children(entry) {
			match id {
				TRACK_TYPE => is_video = be(value) == 1,
				CODEC_ID => header.codec = Some(codec(&String::from_utf8_lossy(value))),
				VIDEO => {
					for (id, value) in children(value) {
						match id {
							PIXEL_WIDTH => header.width = u32::try_from(be(value)).ok(),
							PIXEL_HEIGHT => header.height = u32::try_from(be(value)).ok(),
							_ => {}
						}
					}
				}
				_ => {}
			}
		}
		is_video.then_some(header)
	}

	fn codec(id: &str) -> String {
		let id = id.trim_end_matches('\0');
		match id {
			"V_MPEG4/ISO/AVC" => "h264",
			"V_MPEGH/ISO/HEVC" => "hevc",
			"V_AV1" => "av1",
			"V_VP8" => "vp8",
			"V_VP9" => "vp9",
			"V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/AP" => "mpeg4",
			"V_MJPEG" => "mjpeg",
			"V_PRORES" => "prores",
			other => return other.trim_start_matches("V_").to_lowercase(),
		}
		.to_string()
	}
}
