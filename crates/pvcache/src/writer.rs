use std::{
	collections::HashMap,
	fs::{File, OpenOptions},
	path::Path,
	sync::atomic::Ordering,
};

use memmap2::{Mmap, MmapMut};
use uuid::Uuid;

use crate::{
	error::{Error, Result},
	layout::{
		self, Geometry, SlotRead, FLAG_OCCUPIED, OFF_CAPACITY, OFF_GENERATION, OFF_SLOT_COUNT,
		SLOT_OFF_FLAGS, SLOT_OFF_SEQ, SLOT_OFF_UUID,
	},
	Frame, TileState,
};

/// Slot capacity a fresh cache file is created with.
pub const DEFAULT_INITIAL_CAPACITY: u64 = 1024;

/// The cache's single writer handle.
///
/// One process holds one `Pvcache` per cache file and performs every write;
/// any number of [`crate::PvcacheReader`] handles in other processes map the
/// same file concurrently. Exclusivity among writers is the caller's contract
/// (the daemon owns the file) — it is not enforced with a lock.
pub struct Pvcache {
	file: File,
	map: MmapMut,
	geometry: Geometry,
	capacity: u64,
	slot_count: u64,
	slots: HashMap<Uuid, u64>,
}

impl Pvcache {
	/// Open the cache at `path`, creating it when missing.
	///
	/// An existing file is adopted only when its header validates and
	/// describes the same envelope geometry; anything else — empty, truncated,
	/// foreign contents, an older format version, or different envelope
	/// dimensions — is reinitialized empty. The cache holds nothing that
	/// cannot be rebaked, so recovery is never attempted.
	///
	/// Reinitialization happens only at open, on the assumption that readers
	/// attach after the writer is up; a reader still mapping a file from a
	/// previous run must reopen it.
	pub fn open_or_create(path: &Path, tile_width: u32, tile_height: u32) -> Result<Self> {
		Self::open_or_create_with_capacity(path, tile_width, tile_height, DEFAULT_INITIAL_CAPACITY)
	}

	/// [`Self::open_or_create`] with an explicit initial slot capacity for a
	/// freshly created file. Adopted files keep their persisted capacity.
	pub fn open_or_create_with_capacity(
		path: &Path,
		tile_width: u32,
		tile_height: u32,
		initial_capacity: u64,
	) -> Result<Self> {
		let geometry = Geometry::new(tile_width, tile_height)?;
		let initial_capacity = initial_capacity.max(1);

		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let file = OpenOptions::new()
			.read(true)
			.write(true)
			.create(true)
			.truncate(false)
			.open(path)?;

		let len = file.metadata()?.len();
		let adopted = if len >= geometry.file_len(1) {
			let probe = unsafe { Mmap::map(&file)? };
			layout::parse_header(&probe).filter(|header| header.geometry == geometry)
		} else {
			None
		};

		match adopted {
			Some(header) => {
				let map = unsafe { MmapMut::map_mut(&file)? };
				let mut cache = Self {
					file,
					map,
					geometry,
					capacity: header.capacity,
					slot_count: header.slot_count,
					slots: HashMap::new(),
				};
				cache.rebuild_index();
				Ok(cache)
			}
			None => {
				file.set_len(0)?;
				file.set_len(geometry.file_len(initial_capacity))?;
				let mut map = unsafe { MmapMut::map_mut(&file)? };
				layout::write_header(&mut map, &geometry, initial_capacity);
				Ok(Self {
					file,
					map,
					geometry,
					capacity: initial_capacity,
					slot_count: 0,
					slots: HashMap::new(),
				})
			}
		}
	}

	/// Store `pixels` for `uuid`, stamped with `version`. The buffer is tight
	/// BGRA8 for `frame`, which must fit the file's envelope; only those bytes
	/// are written, so the rest of the slot stays sparse. An entry already in
	/// the cache is rewritten in place under its seqlock; a new entry claims
	/// the next free slot, growing the file when full.
	pub fn write(&mut self, uuid: Uuid, version: u64, frame: Frame, pixels: &[u8]) -> Result<()> {
		if !self.geometry.holds(&frame) {
			return Err(Error::FrameOutOfEnvelope {
				width: frame.content_width,
				height: frame.content_height,
				envelope_width: self.geometry.tile_width,
				envelope_height: self.geometry.tile_height,
			});
		}
		let expected = frame.len();
		if pixels.len() != expected {
			return Err(Error::TileLengthMismatch {
				expected,
				got: pixels.len(),
			});
		}

		if let Some(&slot) = self.slots.get(&uuid) {
			layout::write_slot(
				self.map.as_mut_ptr(),
				&self.geometry,
				slot,
				uuid,
				version,
				&frame,
				pixels,
			);
			return Ok(());
		}

		if self.slot_count == self.capacity {
			self.grow()?;
		}
		let slot = self.slot_count;
		layout::write_slot(
			self.map.as_mut_ptr(),
			&self.geometry,
			slot,
			uuid,
			version,
			&frame,
			pixels,
		);

		// Publish the slot only after its record is complete: readers learn of
		// new records through this counter (acquire load on their side), so a
		// record below the published count is always fully written.
		self.slot_count += 1;
		unsafe { layout::atomic_u64(self.map.as_ptr(), OFF_SLOT_COUNT) }
			.store(self.slot_count, Ordering::Release);
		self.slots.insert(uuid, slot);
		Ok(())
	}

	/// Copy the tile for `uuid` into `buf` (sized for the envelope, exactly
	/// [`Self::tile_len`] bytes) and compare its stored version against
	/// `expected_version`. Both fresh and stale hits fill the buffer, so a
	/// caller can show the stale tile while rebaking; the returned state
	/// carries the frame describing how much of `buf` is valid.
	pub fn get(&self, uuid: Uuid, expected_version: u64, buf: &mut [u8]) -> Result<TileState> {
		let expected = self.geometry.tile_len();
		if buf.len() != expected {
			return Err(Error::TileLengthMismatch {
				expected,
				got: buf.len(),
			});
		}
		let Some(&slot) = self.slots.get(&uuid) else {
			return Ok(TileState::Absent);
		};
		Ok(resolve(
			layout::read_slot(self.map.as_ptr(), &self.geometry, slot, uuid, Some(buf)),
			expected_version,
		))
	}

	/// Compare the stored version for `uuid` against `expected_version`
	/// without copying pixels.
	pub fn lookup(&self, uuid: Uuid, expected_version: u64) -> TileState {
		let Some(&slot) = self.slots.get(&uuid) else {
			return TileState::Absent;
		};
		resolve(
			layout::read_slot(self.map.as_ptr(), &self.geometry, slot, uuid, None),
			expected_version,
		)
	}

	pub fn contains(&self, uuid: Uuid) -> bool {
		self.slots.contains_key(&uuid)
	}

	/// Number of entries currently indexed.
	pub fn len(&self) -> usize {
		self.slots.len()
	}

	pub fn is_empty(&self) -> bool {
		self.slots.is_empty()
	}

	pub fn capacity(&self) -> u64 {
		self.capacity
	}

	/// Envelope width: the widest frame a slot in this file can hold.
	pub fn tile_width(&self) -> u32 {
		self.geometry.tile_width
	}

	/// Envelope height: the tallest frame a slot in this file can hold.
	pub fn tile_height(&self) -> u32 {
		self.geometry.tile_height
	}

	/// Bytes a full-envelope tile occupies, and the buffer size [`Self::get`]
	/// expects: `tile_width * tile_height * 4`.
	pub fn tile_len(&self) -> usize {
		self.geometry.tile_len()
	}

	/// Ask the OS to write dirty pages back to disk.
	pub fn flush(&self) -> Result<()> {
		self.map.flush()?;
		Ok(())
	}

	/// Double the slot capacity: extend the file (existing slots never move),
	/// remap, then publish the new capacity and bump the generation so
	/// readers remap. `set_len` gives a sparse file on filesystems that
	/// support it, so unwritten slots cost no disk.
	fn grow(&mut self) -> Result<()> {
		let new_capacity = self
			.capacity
			.checked_mul(2)
			.filter(|c| self.geometry.file_len(*c) <= i64::MAX as u64)
			.ok_or(Error::CapacityOverflow)?;
		self.map.flush()?;
		self.file.set_len(self.geometry.file_len(new_capacity))?;
		self.map = unsafe { MmapMut::map_mut(&self.file)? };

		let base = self.map.as_ptr();
		unsafe { layout::atomic_u64(base, OFF_CAPACITY) }.store(new_capacity, Ordering::Release);
		unsafe { layout::atomic_u64(base, OFF_GENERATION) }.fetch_add(1, Ordering::Release);
		self.capacity = new_capacity;
		Ok(())
	}

	/// Rebuild the uuid → slot index from the persisted slot records. A record
	/// whose sequence word is still odd is a write that never completed (power
	/// loss mid-bake); its pixels may be torn, so it is retired — unoccupied,
	/// sequence settled — and the entry rebakes into a fresh slot later.
	fn rebuild_index(&mut self) {
		let base = self.map.as_mut_ptr();
		for slot in 0..self.slot_count {
			let record = self.geometry.slot_offset(slot) as usize;
			let seq = unsafe { layout::atomic_u32(base, record + SLOT_OFF_SEQ) };
			let observed = seq.load(Ordering::Acquire);
			if observed & 1 == 1 {
				unsafe { (base.add(record + SLOT_OFF_FLAGS) as *mut u32).write_volatile(0) };
				seq.store(observed.wrapping_add(1), Ordering::Release);
				continue;
			}
			let (flags, key) = unsafe {
				(
					(base.add(record + SLOT_OFF_FLAGS) as *const u32).read_volatile(),
					(base.add(record + SLOT_OFF_UUID) as *const [u8; 16]).read_volatile(),
				)
			};
			if flags & FLAG_OCCUPIED != 0 {
				self.slots.insert(Uuid::from_bytes(key), slot);
			}
		}
	}
}

impl Drop for Pvcache {
	fn drop(&mut self) {
		let _ = self.map.flush();
	}
}

pub(crate) fn resolve(read: SlotRead, expected_version: u64) -> TileState {
	match read {
		SlotRead::Tile { version, frame } if version == expected_version => {
			TileState::Fresh { frame }
		}
		SlotRead::Tile { version, frame } => TileState::Stale { version, frame },
		SlotRead::Empty | SlotRead::Busy => TileState::Absent,
	}
}
