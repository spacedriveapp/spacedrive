use sd_core_prisma_helpers::CasId;

use std::path::Path;

use blake3::Hasher;
use static_assertions::const_assert;
use tokio::{
	fs::{self, File},
	io::{self, AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tracing::{instrument, trace, Level};

const SAMPLE_COUNT: u64 = 4;
const SAMPLE_SIZE: u64 = 1024 * 10;
const HEADER_OR_FOOTER_SIZE: u64 = 1024 * 8;

// minimum file size of 100KiB, to avoid sample hashing for small files as they can be smaller than the total sample size
const MINIMUM_FILE_SIZE: u64 = 1024 * 100;

// Asserting that nobody messed up our consts
const_assert!((HEADER_OR_FOOTER_SIZE * 2 + SAMPLE_COUNT * SAMPLE_SIZE) < MINIMUM_FILE_SIZE);

// Asserting that the sample size is larger than header/footer size, as the same buffer is used for both
const_assert!(SAMPLE_SIZE > HEADER_OR_FOOTER_SIZE);

#[instrument(
	skip(path),
	ret(level = Level::TRACE),
	err,
	fields(path = %path.as_ref().display()
))]
// SAFETY: Casts here are safe, they're hardcoded values we have some const assertions above to make sure they're correct
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub async fn generate_cas_id(
	path: impl AsRef<Path> + Send,
	size: u64,
) -> Result<CasId<'static>, io::Error> {
	let mut hasher = Hasher::new();
	hasher.update(&size.to_le_bytes());

	if size <= MINIMUM_FILE_SIZE {
		trace!("File is small, hashing the whole file");
		// For small files, we hash the whole file
		hasher.update(&fs::read(path).await?);
	} else {
		trace!("File bigger than threshold, hashing samples");

		let mut file = File::open(path).await?;
		let mut buf = vec![0; SAMPLE_SIZE as usize].into_boxed_slice();

		// Hashing the header
		let mut current_pos = file
			.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await? as u64;
		hasher.update(&buf[..HEADER_OR_FOOTER_SIZE as usize]);

		// Sample hashing the inner content of the file
		let seek_jump = (size - HEADER_OR_FOOTER_SIZE * 2) / SAMPLE_COUNT;
		loop {
			file.read_exact(&mut buf).await?;
			hasher.update(&buf);

			if current_pos >= (HEADER_OR_FOOTER_SIZE + seek_jump * (SAMPLE_COUNT - 1)) {
				break;
			}

			current_pos = file.seek(SeekFrom::Start(current_pos + seek_jump)).await?;
		}

		// Hashing the footer
		file.seek(SeekFrom::End(-(HEADER_OR_FOOTER_SIZE as i64)))
			.await?;
		file.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await?;
		hasher.update(&buf[..HEADER_OR_FOOTER_SIZE as usize]);
	}

	Ok(hasher.finalize().to_hex()[..16].to_string().into())
}
