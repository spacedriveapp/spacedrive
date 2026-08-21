use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
	#[error("i/o error: {0}")]
	Io(#[from] std::io::Error),

	/// The file at the given path is not a usable cache: missing header,
	/// wrong magic, an unsupported format version, or contents truncated
	/// below what its header describes. The cache is disposable, so callers
	/// treat this as "no cache" rather than a failure to recover from.
	#[error("not a pvcache file (empty, truncated, or unsupported format)")]
	Incompatible,

	#[error("tile dimensions out of range: {width}x{height}")]
	InvalidTileDimensions { width: u32, height: u32 },

	#[error("pixel buffer length {got} does not match tile length {expected}")]
	TileLengthMismatch { expected: usize, got: usize },

	#[error("slot capacity would overflow")]
	CapacityOverflow,
}
