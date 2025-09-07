use anyhow::{Context, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::fs::File;
use std::path::PathBuf;

use super::DatasetGenerator;
use crate::recipe::{ContentGen, ContentMode, FileSpec, Recipe, SizeBucket};

#[derive(Debug, Default)]
pub struct FileSystemGenerator;

fn choose_depth(rng: &mut StdRng, max_depth: usize) -> usize {
	if max_depth == 0 {
		1
	} else {
		rng.gen_range(1..=max_depth)
	}
}

fn write_magic_header_if_needed(
	file: &mut File,
	extension: &str,
	enable_magic: bool,
) -> Result<usize> {
	if !enable_magic {
		return Ok(0);
	}
	let registry = sd_core_new::file_type::FileTypeRegistry::new();
	let mut candidates = registry.get_by_extension(extension);
	if candidates.is_empty() {
		return Ok(0);
	}
	// Choose highest priority candidate with magic bytes; fallback to none
	candidates.sort_by_key(|ft| std::cmp::Reverse(ft.priority));
	let file_type = match candidates.into_iter().find(|ft| !ft.magic_bytes.is_empty()) {
		Some(ft) => ft,
		None => return Ok(0),
	};

	// Write the first magic pattern at its specified offset
	if let Some(pattern) = file_type.magic_bytes.first() {
		use std::io::{Seek, SeekFrom, Write};
		// Ensure file is at start
		file.seek(SeekFrom::Start(0))?;
		// If offset > 0, pad zeros up to offset
		if pattern.offset > 0 {
			let padding = vec![0u8; pattern.offset];
			file.write_all(&padding)?;
		}
		// Convert MagicByte to concrete bytes (wildcards -> 0)
		let bytes: Vec<u8> = pattern
			.bytes
			.iter()
			.map(|b| match b {
				sd_core_new::file_type::MagicByte::Exact(v) => *v,
				sd_core_new::file_type::MagicByte::Any => 0u8,
				sd_core_new::file_type::MagicByte::Range { min, .. } => *min,
			})
			.collect();
		file.write_all(&bytes)?;
		return Ok(pattern.offset + bytes.len());
	}
	Ok(0)
}

fn write_content_for_hashing(
	file: &mut File,
	total_size: u64,
	content_gen: Option<&ContentGen>,
	extension: &str,
) -> Result<()> {
	use std::io::{Seek, SeekFrom, Write};
	let cfg = content_gen;

	// Default: sparse zeros
	let mut wrote_up_to: u64 = 0;

	let (mode, sample_block_size, magic_headers) = match cfg {
		Some(c) => (c.mode.clone(), c.sample_block_size, c.magic_headers),
		None => (ContentMode::Zeros, 10 * 1024, false),
	};

	// Write magic header first if requested
	if magic_headers {
		let header_end = write_magic_header_if_needed(file, extension, true)? as u64;
		wrote_up_to = wrote_up_to.max(header_end);
	}

	match mode {
		ContentMode::Zeros => {
			// Sparse: just set_len
			file.set_len(total_size)?;
		}
		ContentMode::Partial => {
			// Follow ContentHashGenerator: header (8KiB), 4 samples of 10KiB, footer (8KiB)
			const HEADER_OR_FOOTER_SIZE: u64 = 8 * 1024;
			const SAMPLE_COUNT: u64 = 4;
			let sample_size = sample_block_size.max(1);
			if total_size <= sd_core_new::domain::content_identity::MINIMUM_FILE_SIZE {
				// small file: write full content deterministically but lighter (10KiB chunks)
				let mut remaining = total_size;
				let mut pos = wrote_up_to;
				while remaining > 0 {
					let chunk = remaining.min(sample_size);
					file.seek(SeekFrom::Start(pos))?;
					file.write_all(&deterministic_bytes(pos, chunk))?;
					pos += chunk;
					remaining -= chunk;
				}
				file.set_len(total_size)?;
			} else {
				// header
				file.seek(SeekFrom::Start(0))?;
				file.write_all(&deterministic_bytes(0, HEADER_OR_FOOTER_SIZE))?;

				// inner samples evenly spaced
				let seek_jump = (total_size - HEADER_OR_FOOTER_SIZE * 2) / SAMPLE_COUNT;
				let mut current_pos = HEADER_OR_FOOTER_SIZE;
				for i in 0..SAMPLE_COUNT {
					file.seek(SeekFrom::Start(current_pos))?;
					file.write_all(&deterministic_bytes(current_pos, sample_size))?;
					if i + 1 == SAMPLE_COUNT {
						break;
					}
					current_pos += seek_jump;
				}

				// footer
				if total_size >= HEADER_OR_FOOTER_SIZE {
					let footer_start = total_size - HEADER_OR_FOOTER_SIZE;
					file.seek(SeekFrom::Start(footer_start))?;
					file.write_all(&deterministic_bytes(footer_start, HEADER_OR_FOOTER_SIZE))?;
				}

				file.set_len(total_size)?;
			}
		}
		ContentMode::Full => {
			// Fill entire file with deterministic bytes
			let mut remaining = total_size;
			let mut pos = wrote_up_to;
			let block = sample_block_size.max(4096);
			while remaining > 0 {
				let chunk = remaining.min(block);
				file.seek(SeekFrom::Start(pos))?;
				file.write_all(&deterministic_bytes(pos, chunk))?;
				pos += chunk;
				remaining -= chunk;
			}
			file.set_len(total_size)?;
		}
	}

	Ok(())
}

fn deterministic_bytes(offset: u64, len: u64) -> Vec<u8> {
	// Derive a deterministic pattern from offset and length to make hashes stable
	// Simple PRF: blake3 hash of (offset||counter) to fill buffer
	use blake3::Hasher;
	let mut out = vec![0u8; len as usize];
	let mut filled = 0usize;
	let mut counter: u64 = 0;
	while filled < out.len() {
		let mut hasher = Hasher::new();
		hasher.update(&offset.to_le_bytes());
		hasher.update(&counter.to_le_bytes());
		let digest = hasher.finalize();
		let bytes = digest.as_bytes();
		let remain = out.len() - filled;
		let take = remain.min(bytes.len());
		out[filled..filled + take].copy_from_slice(&bytes[..take]);
		filled += take;
		counter += 1;
	}
	out
}

fn choose_fanout_index(rng: &mut StdRng, fanout_per_dir: usize) -> usize {
	if fanout_per_dir == 0 {
		0
	} else {
		rng.gen_range(0..fanout_per_dir)
	}
}

fn pick_size(rng: &mut StdRng, range: [u64; 2]) -> u64 {
	let [min_b, max_b] = range;
	if max_b > min_b {
		rng.gen_range(min_b..=max_b)
	} else {
		min_b
	}
}

fn allocate_buckets(buckets: &[(String, SizeBucket)], originals_target: u64) -> Vec<u64> {
	let share_sum: f32 = buckets.iter().map(|(_, b)| b.share.max(0.0)).sum();
	let norm = if share_sum > 0.0 { share_sum } else { 1.0 };
	let mut result = Vec::with_capacity(buckets.len());
	let mut assigned = 0u64;
	for (i, (_, b)) in buckets.iter().enumerate() {
		let portion = (b.share.max(0.0) / norm) as f64;
		let mut count = (portion * (originals_target as f64)).floor() as u64;
		if i == buckets.len() - 1 {
			count = originals_target.saturating_sub(assigned);
		}
		assigned += count;
		result.push(count);
	}
	result
}

#[async_trait::async_trait]
impl DatasetGenerator for FileSystemGenerator {
	fn name(&self) -> &'static str {
		"filesystem"
	}

	async fn generate(&self, recipe: &Recipe) -> Result<()> {
		let mut rng: StdRng = match recipe.seed {
			Some(s) => StdRng::seed_from_u64(s),
			None => StdRng::from_entropy(),
		};

		for loc in &recipe.locations {
			std::fs::create_dir_all(&loc.path).with_context(|| format!("create {:?}", loc.path))?;

			let total_files = loc.files.total as u64;
			let dup_ratio = loc.files.duplicate_ratio.unwrap_or(0.0).clamp(0.0, 0.95) as f64;
			let originals_target = ((total_files as f64) * (1.0 - dup_ratio)).round() as u64;
			let duplicates_target = total_files.saturating_sub(originals_target);

			let mut buckets: Vec<(String, SizeBucket)> = loc
				.files
				.size_buckets
				.iter()
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect();
			buckets.sort_by(|a, b| a.0.cmp(&b.0));

			let bucket_counts = allocate_buckets(&buckets, originals_target);

			let extensions: Vec<String> = loc
				.files
				.extensions
				.clone()
				.unwrap_or_else(|| vec!["bin".to_string()]);

			let mut created_files: Vec<PathBuf> = Vec::with_capacity(originals_target as usize);

			for ((_, bucket), count) in buckets.iter().zip(bucket_counts.iter()) {
				for _ in 0..*count {
					let mut dir = loc.path.clone();
					let depth = choose_depth(&mut rng, loc.structure.depth);
					for _ in 0..depth {
						let idx = choose_fanout_index(&mut rng, loc.structure.fanout_per_dir);
						dir = dir.join(format!("d{}", idx));
					}
					std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {:?}", dir))?;

					let ext = &extensions[rng.gen_range(0..extensions.len())];
					let fname = format!("f_{:016x}.{}", rng.gen::<u64>(), ext);
					let fpath = dir.join(fname);
					let size = pick_size(&mut rng, bucket.range);

					let mut file =
						File::create(&fpath).with_context(|| format!("create {:?}", fpath))?;
					// Write content based on content_gen settings
					write_content_for_hashing(
						&mut file,
						size,
						loc.files.content_gen.as_ref(),
						ext,
					)?;
					created_files.push(fpath);
				}
			}

			for _ in 0..duplicates_target {
				if created_files.is_empty() {
					break;
				}
				let src_idx = rng.gen_range(0..created_files.len());
				let src = &created_files[src_idx];
				let mut dir = loc.path.clone();
				let depth = choose_depth(&mut rng, loc.structure.depth);
				for _ in 0..depth {
					let idx = choose_fanout_index(&mut rng, loc.structure.fanout_per_dir);
					dir = dir.join(format!("d{}", idx));
				}
				std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {:?}", dir))?;
				let ext = src
					.extension()
					.map(|e| format!(".{}", e.to_string_lossy()))
					.unwrap_or_default();
				let dst = dir.join(format!("dup_{:016x}{}", rng.gen::<u64>(), ext));
				match std::fs::hard_link(src, &dst) {
					Ok(_) => {}
					Err(_) => {
						std::fs::copy(src, &dst)
							.with_context(|| format!("copy {:?} -> {:?}", src, dst))?;
					}
				}
			}
			// After generation for this location, write a marker file
			let marker = loc.path.join(".sd-bench-generated");
			if let Err(e) = std::fs::write(&marker, b"ok") {
				eprintln!(
					"Warning: failed to write generation marker at {}: {}",
					marker.display(),
					e
				);
			}
		}

		Ok(())
	}
}
