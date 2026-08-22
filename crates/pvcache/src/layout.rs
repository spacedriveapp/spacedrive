//! On-disk layout: header field offsets, slot record geometry, and the
//! seqlock-guarded record accessors shared by the writer and reader handles.

use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};

use uuid::Uuid;

use crate::error::{Error, Result};
use crate::Frame;

pub(crate) const MAGIC: [u8; 8] = *b"SDPVCACH";
pub(crate) const FORMAT_VERSION: u32 = 2;
pub(crate) const PIXEL_FORMAT_BGRA8: u32 = 0;

/// Bytes reserved at the start of the file for the header. Slot records begin
/// here, page-aligned, leaving room for future header fields without a layout
/// change.
pub(crate) const HEADER_RESERVED: u64 = 4096;

/// Bytes of record header preceding each slot's pixels.
pub(crate) const SLOT_HEADER_LEN: u64 = 64;

/// Slot strides are rounded up to this alignment so the record header's
/// atomics stay naturally aligned for every tile geometry.
const SLOT_ALIGN: u64 = 64;

pub(crate) const BYTES_PER_PIXEL: u64 = 4;
pub(crate) const MAX_TILE_DIM: u32 = 8192;

// Header field offsets. Fields through `slot_stride` are written once at
// creation; `capacity`, `slot_count`, and `generation` are updated through
// atomics while readers in other processes map the same page.
const OFF_MAGIC: usize = 0;
const OFF_FORMAT_VERSION: usize = 8;
const OFF_PIXEL_FORMAT: usize = 12;
const OFF_TILE_WIDTH: usize = 16;
const OFF_TILE_HEIGHT: usize = 20;
const OFF_SLOT_STRIDE: usize = 24;
pub(crate) const OFF_CAPACITY: usize = 32;
pub(crate) const OFF_SLOT_COUNT: usize = 40;
pub(crate) const OFF_GENERATION: usize = 48;

// Slot record field offsets, relative to the record start.
pub(crate) const SLOT_OFF_SEQ: usize = 0;
pub(crate) const SLOT_OFF_FLAGS: usize = 4;
pub(crate) const SLOT_OFF_VERSION: usize = 8;
pub(crate) const SLOT_OFF_UUID: usize = 16;
pub(crate) const SLOT_OFF_CONTENT_W: usize = 32;
pub(crate) const SLOT_OFF_CONTENT_H: usize = 34;
pub(crate) const SLOT_OFF_SOURCE_W: usize = 36;
pub(crate) const SLOT_OFF_SOURCE_H: usize = 40;

pub(crate) const FLAG_OCCUPIED: u32 = 1;

/// Attempts before a slot read gives up on a record whose sequence word never
/// settles (a writer that died mid-write leaves the odd value behind forever).
const READ_RETRIES: usize = 4096;

/// Tile geometry and the derived slot stride. Fixed per cache file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
	pub tile_width: u32,
	pub tile_height: u32,
	pub slot_stride: u64,
}

impl Geometry {
	pub fn new(tile_width: u32, tile_height: u32) -> Result<Self> {
		if tile_width == 0
			|| tile_height == 0
			|| tile_width > MAX_TILE_DIM
			|| tile_height > MAX_TILE_DIM
		{
			return Err(Error::InvalidTileDimensions {
				width: tile_width,
				height: tile_height,
			});
		}
		let tile_len = tile_width as u64 * tile_height as u64 * BYTES_PER_PIXEL;
		let slot_stride = (SLOT_HEADER_LEN + tile_len).div_ceil(SLOT_ALIGN) * SLOT_ALIGN;
		Ok(Self {
			tile_width,
			tile_height,
			slot_stride,
		})
	}

	pub fn tile_len(&self) -> usize {
		(self.tile_width as u64 * self.tile_height as u64 * BYTES_PER_PIXEL) as usize
	}

	pub fn slot_offset(&self, slot: u64) -> u64 {
		HEADER_RESERVED + slot * self.slot_stride
	}

	pub fn file_len(&self, capacity: u64) -> u64 {
		self.slot_offset(capacity)
	}

	/// Whether a frame's extent fits this file's envelope.
	pub fn holds(&self, frame: &Frame) -> bool {
		frame.content_width > 0
			&& frame.content_height > 0
			&& frame.content_width <= self.tile_width
			&& frame.content_height <= self.tile_height
	}
}

/// Header contents as validated at open time. The mutable fields are
/// re-read through atomics afterwards.
pub(crate) struct ParsedHeader {
	pub geometry: Geometry,
	pub capacity: u64,
	pub slot_count: u64,
	pub generation: u64,
}

/// Stamp a fresh header into a newly created mapping.
pub(crate) fn write_header(map: &mut [u8], geometry: &Geometry, capacity: u64) {
	map[..HEADER_RESERVED as usize].fill(0);
	map[OFF_MAGIC..OFF_MAGIC + 8].copy_from_slice(&MAGIC);
	put_u32(map, OFF_FORMAT_VERSION, FORMAT_VERSION);
	put_u32(map, OFF_PIXEL_FORMAT, PIXEL_FORMAT_BGRA8);
	put_u32(map, OFF_TILE_WIDTH, geometry.tile_width);
	put_u32(map, OFF_TILE_HEIGHT, geometry.tile_height);
	put_u64(map, OFF_SLOT_STRIDE, geometry.slot_stride);
	put_u64(map, OFF_CAPACITY, capacity);
	put_u64(map, OFF_SLOT_COUNT, 0);
	put_u64(map, OFF_GENERATION, 0);
}

/// Validate a mapped file's header. `None` means the file is not a usable
/// cache (bad magic, unsupported version, or truncated below what the header
/// describes) — the writer reinitializes it, the reader reports it absent.
pub(crate) fn parse_header(map: &[u8]) -> Option<ParsedHeader> {
	if map.len() < HEADER_RESERVED as usize {
		return None;
	}
	if map[OFF_MAGIC..OFF_MAGIC + 8] != MAGIC {
		return None;
	}
	if get_u32(map, OFF_FORMAT_VERSION) != FORMAT_VERSION {
		return None;
	}
	if get_u32(map, OFF_PIXEL_FORMAT) != PIXEL_FORMAT_BGRA8 {
		return None;
	}
	let geometry =
		Geometry::new(get_u32(map, OFF_TILE_WIDTH), get_u32(map, OFF_TILE_HEIGHT)).ok()?;
	if get_u64(map, OFF_SLOT_STRIDE) != geometry.slot_stride {
		return None;
	}
	// The mutable fields may be racing a live writer; read them atomically.
	let base = map.as_ptr();
	let capacity = unsafe { atomic_u64(base, OFF_CAPACITY) }.load(Ordering::Acquire);
	let slot_count = unsafe { atomic_u64(base, OFF_SLOT_COUNT) }.load(Ordering::Acquire);
	let generation = unsafe { atomic_u64(base, OFF_GENERATION) }.load(Ordering::Acquire);
	if capacity == 0 || slot_count > capacity {
		return None;
	}
	if (map.len() as u64) < geometry.file_len(capacity) {
		return None;
	}
	Some(ParsedHeader {
		geometry,
		capacity,
		slot_count,
		generation,
	})
}

/// # Safety
/// `base + offset` must lie within a live mapping and be 4-byte aligned.
pub(crate) unsafe fn atomic_u32<'a>(base: *const u8, offset: usize) -> &'a AtomicU32 {
	&*(base.add(offset) as *const AtomicU32)
}

/// # Safety
/// `base + offset` must lie within a live mapping and be 8-byte aligned.
pub(crate) unsafe fn atomic_u64<'a>(base: *const u8, offset: usize) -> &'a AtomicU64 {
	&*(base.add(offset) as *const AtomicU64)
}

/// Outcome of a seqlock-guarded slot read.
pub(crate) enum SlotRead {
	/// The record holds the requested uuid; `version` is its stored content
	/// version, `frame` the extent of its pixels, and, when a buffer was
	/// supplied, those pixels were copied into it as a consistent snapshot.
	Tile { version: u64, frame: Frame },
	/// The record is unoccupied or holds a different uuid.
	Empty,
	/// The record's sequence word never settled — a write is in flight (or a
	/// writer died mid-write). Indistinguishable from absent for callers.
	Busy,
}

/// Read a slot record under its seqlock, optionally copying the pixels.
///
/// The sequence word is sampled (acquire) before the fields are read and
/// re-checked after; a change or an odd value means the read raced a rewrite
/// and is retried. Pixels are copied with `copy_nonoverlapping` and validated
/// by the same re-check before the copy is trusted. Only the stored frame's
/// bytes are copied, so `pixels_out` beyond `frame.len()` is left as the
/// caller supplied it.
pub(crate) fn read_slot(
	base: *const u8,
	geometry: &Geometry,
	slot: u64,
	uuid: Uuid,
	mut pixels_out: Option<&mut [u8]>,
) -> SlotRead {
	let record = geometry.slot_offset(slot) as usize;
	let seq = unsafe { atomic_u32(base, record + SLOT_OFF_SEQ) };
	for _ in 0..READ_RETRIES {
		let observed = seq.load(Ordering::Acquire);
		if observed & 1 == 1 {
			std::hint::spin_loop();
			continue;
		}
		let (flags, version, key, content_w, content_h, source_w, source_h) = unsafe {
			(
				(base.add(record + SLOT_OFF_FLAGS) as *const u32).read_volatile(),
				(base.add(record + SLOT_OFF_VERSION) as *const u64).read_volatile(),
				(base.add(record + SLOT_OFF_UUID) as *const [u8; 16]).read_volatile(),
				(base.add(record + SLOT_OFF_CONTENT_W) as *const u16).read_volatile(),
				(base.add(record + SLOT_OFF_CONTENT_H) as *const u16).read_volatile(),
				(base.add(record + SLOT_OFF_SOURCE_W) as *const u32).read_volatile(),
				(base.add(record + SLOT_OFF_SOURCE_H) as *const u32).read_volatile(),
			)
		};
		fence(Ordering::Acquire);
		if seq.load(Ordering::Relaxed) != observed {
			continue;
		}
		if flags & FLAG_OCCUPIED == 0 || key != *uuid.as_bytes() {
			return SlotRead::Empty;
		}
		let frame = Frame {
			content_width: u32::from(content_w),
			content_height: u32::from(content_h),
			source_width: source_w,
			source_height: source_h,
		};
		// An extent outside the envelope belongs to a record caught mid-write;
		// the re-check below would reject it anyway, but the copy length is
		// derived from it, so it is validated before it is used.
		if !geometry.holds(&frame) {
			continue;
		}
		if let Some(buf) = pixels_out.as_deref_mut() {
			let len = frame.len();
			if len > buf.len() {
				continue;
			}
			unsafe {
				std::ptr::copy_nonoverlapping(
					base.add(record + SLOT_HEADER_LEN as usize),
					buf.as_mut_ptr(),
					len,
				);
			}
			fence(Ordering::Acquire);
			if seq.load(Ordering::Relaxed) != observed {
				continue;
			}
		}
		return SlotRead::Tile { version, frame };
	}
	SlotRead::Busy
}

/// Write a slot record and its pixels under the seqlock.
///
/// The sequence word goes odd before any field is touched (the acquire RMW
/// keeps the writes below from being reordered ahead of it) and returns to
/// even with release ordering once the record is complete, so a reader that
/// observes an even, unchanged word has seen a consistent record.
pub(crate) fn write_slot(
	base: *mut u8,
	geometry: &Geometry,
	slot: u64,
	uuid: Uuid,
	version: u64,
	frame: &Frame,
	pixels: &[u8],
) {
	let record = geometry.slot_offset(slot) as usize;
	let seq = unsafe { atomic_u32(base as *const u8, record + SLOT_OFF_SEQ) };
	let opened = seq.fetch_add(1, Ordering::Acquire);
	debug_assert_eq!(opened & 1, 0, "slot record already mid-write");
	unsafe {
		(base.add(record + SLOT_OFF_FLAGS) as *mut u32).write_volatile(FLAG_OCCUPIED);
		(base.add(record + SLOT_OFF_VERSION) as *mut u64).write_volatile(version);
		(base.add(record + SLOT_OFF_UUID) as *mut [u8; 16]).write_volatile(*uuid.as_bytes());
		(base.add(record + SLOT_OFF_CONTENT_W) as *mut u16)
			.write_volatile(frame.content_width as u16);
		(base.add(record + SLOT_OFF_CONTENT_H) as *mut u16)
			.write_volatile(frame.content_height as u16);
		(base.add(record + SLOT_OFF_SOURCE_W) as *mut u32).write_volatile(frame.source_width);
		(base.add(record + SLOT_OFF_SOURCE_H) as *mut u32).write_volatile(frame.source_height);
		std::ptr::copy_nonoverlapping(
			pixels.as_ptr(),
			base.add(record + SLOT_HEADER_LEN as usize),
			pixels.len(),
		);
	}
	seq.store(opened.wrapping_add(2), Ordering::Release);
}

fn put_u32(map: &mut [u8], offset: usize, value: u32) {
	map[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(map: &mut [u8], offset: usize, value: u64) {
	map[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u32(map: &[u8], offset: usize) -> u32 {
	u32::from_le_bytes(map[offset..offset + 4].try_into().expect("4-byte field"))
}

fn get_u64(map: &[u8], offset: usize) -> u64 {
	u64::from_le_bytes(map[offset..offset + 8].try_into().expect("8-byte field"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn slot_stride_is_aligned_and_fits_header_plus_tile() {
		let geometry = Geometry::new(128, 128).expect("valid dims");
		assert_eq!(geometry.tile_len(), 128 * 128 * 4);
		assert_eq!(geometry.slot_stride % SLOT_ALIGN, 0);
		assert!(geometry.slot_stride >= SLOT_HEADER_LEN + geometry.tile_len() as u64);

		// A tile length that is not a multiple of the alignment still rounds up.
		let odd = Geometry::new(3, 3).expect("valid dims");
		assert_eq!(odd.slot_stride % SLOT_ALIGN, 0);
		assert!(odd.slot_stride >= SLOT_HEADER_LEN + odd.tile_len() as u64);
	}

	#[test]
	fn rejects_degenerate_tile_dimensions() {
		assert!(Geometry::new(0, 128).is_err());
		assert!(Geometry::new(128, 0).is_err());
		assert!(Geometry::new(MAX_TILE_DIM + 1, 128).is_err());
	}
}
