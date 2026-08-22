use std::{
	collections::HashMap,
	fs::{File, OpenOptions},
	path::Path,
	sync::atomic::Ordering,
};

use memmap2::Mmap;
use uuid::Uuid;

use crate::{
	error::{Error, Result},
	layout::{
		self, Geometry, FLAG_OCCUPIED, HEADER_RESERVED, OFF_GENERATION, OFF_SLOT_COUNT,
		SLOT_OFF_FLAGS, SLOT_OFF_UUID,
	},
	writer::resolve,
	TileState,
};

/// A read-only mapping of a cache file owned by a writer elsewhere — typically
/// another process. Opening never modifies the file.
///
/// The handle tracks the writer through the shared header: new entries are
/// discovered via the published slot count, and a generation change (the file
/// grew) triggers a remap. Existing slots never move, so tiles read from an
/// older mapping remain valid.
pub struct PvcacheReader {
	file: File,
	map: Mmap,
	geometry: Geometry,
	generation: u64,
	scanned: u64,
	slots: HashMap<Uuid, u64>,
}

impl PvcacheReader {
	/// Open the cache file at `path` read-only.
	///
	/// A missing file surfaces as [`Error::Io`]; an empty, truncated, or
	/// unrecognized file as [`Error::Incompatible`]. Both mean "no cache" —
	/// the cache is disposable and a writer will recreate it.
	pub fn open(path: &Path) -> Result<Self> {
		let file = OpenOptions::new().read(true).open(path)?;
		if file.metadata()?.len() < HEADER_RESERVED {
			return Err(Error::Incompatible);
		}
		let map = unsafe { Mmap::map(&file)? };
		let header = layout::parse_header(&map).ok_or(Error::Incompatible)?;
		let mut reader = Self {
			file,
			map,
			geometry: header.geometry,
			generation: header.generation,
			scanned: 0,
			slots: HashMap::new(),
		};
		reader.scan();
		Ok(reader)
	}

	/// Copy the tile for `uuid` into `buf` (sized for the envelope, exactly
	/// [`Self::tile_len`] bytes) and compare its stored version against
	/// `expected_version`. Both fresh and stale hits fill the buffer, and the
	/// returned state carries the frame describing how much of it is valid.
	/// Takes `&mut self` because the handle may remap the grown file and
	/// extend its slot index.
	pub fn get(&mut self, uuid: Uuid, expected_version: u64, buf: &mut [u8]) -> Result<TileState> {
		let expected = self.geometry.tile_len();
		if buf.len() != expected {
			return Err(Error::TileLengthMismatch {
				expected,
				got: buf.len(),
			});
		}
		self.sync()?;
		let Some(&slot) = self.slots.get(&uuid) else {
			return Ok(TileState::Absent);
		};
		if self.geometry.slot_offset(slot + 1) > self.map.len() as u64 {
			return Ok(TileState::Absent);
		}
		Ok(resolve(
			layout::read_slot(self.map.as_ptr(), &self.geometry, slot, uuid, Some(buf)),
			expected_version,
		))
	}

	/// Compare the stored version for `uuid` against `expected_version`
	/// without copying pixels.
	pub fn lookup(&mut self, uuid: Uuid, expected_version: u64) -> Result<TileState> {
		self.sync()?;
		let Some(&slot) = self.slots.get(&uuid) else {
			return Ok(TileState::Absent);
		};
		if self.geometry.slot_offset(slot + 1) > self.map.len() as u64 {
			return Ok(TileState::Absent);
		}
		Ok(resolve(
			layout::read_slot(self.map.as_ptr(), &self.geometry, slot, uuid, None),
			expected_version,
		))
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

	/// Number of entries currently indexed. Grows as the writer publishes new
	/// slots and this handle observes them.
	pub fn len(&self) -> usize {
		self.slots.len()
	}

	pub fn is_empty(&self) -> bool {
		self.slots.is_empty()
	}

	/// Catch up with the writer: remap if the file grew, then index any newly
	/// published slot records.
	fn sync(&mut self) -> Result<()> {
		let generation = unsafe { layout::atomic_u64(self.map.as_ptr(), OFF_GENERATION) }
			.load(Ordering::Acquire);
		if generation != self.generation {
			self.remap()?;
		}
		self.scan();
		Ok(())
	}

	fn remap(&mut self) -> Result<()> {
		let map = unsafe { Mmap::map(&self.file)? };
		let header = layout::parse_header(&map).ok_or(Error::Incompatible)?;
		if header.geometry != self.geometry {
			return Err(Error::Incompatible);
		}
		self.generation = header.generation;
		self.map = map;
		Ok(())
	}

	/// Index slot records published since the last scan. The writer stores the
	/// slot count with release ordering after a record's first write completes,
	/// so every record below the acquired count holds a settled uuid; later
	/// rewrites change only version and pixels, never the key.
	fn scan(&mut self) {
		let base = self.map.as_ptr();
		let published = unsafe { layout::atomic_u64(base, OFF_SLOT_COUNT) }.load(Ordering::Acquire);
		// Records past the current mapping (published between the generation
		// check and here) are picked up after the next remap.
		let mapped =
			(self.map.len() as u64).saturating_sub(HEADER_RESERVED) / self.geometry.slot_stride;
		let limit = published.min(mapped);
		while self.scanned < limit {
			let record = self.geometry.slot_offset(self.scanned) as usize;
			let (flags, key) = unsafe {
				(
					(base.add(record + SLOT_OFF_FLAGS) as *const u32).read_volatile(),
					(base.add(record + SLOT_OFF_UUID) as *const [u8; 16]).read_volatile(),
				)
			};
			if flags & FLAG_OCCUPIED != 0 {
				self.slots.insert(Uuid::from_bytes(key), self.scanned);
			}
			self.scanned += 1;
		}
	}
}
