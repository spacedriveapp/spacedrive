use anyhow::{Context, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::fs::File;
use std::path::PathBuf;

use crate::recipe::{FileSpec, Recipe, SizeBucket};
use super::DatasetGenerator;

#[derive(Debug, Default)]
pub struct FileSystemGenerator;

fn choose_depth(rng: &mut StdRng, max_depth: usize) -> usize {
    if max_depth == 0 { 1 } else { rng.gen_range(1..=max_depth) }
}

fn choose_fanout_index(rng: &mut StdRng, fanout_per_dir: usize) -> usize {
    if fanout_per_dir == 0 { 0 } else { rng.gen_range(0..fanout_per_dir) }
}

fn pick_size(rng: &mut StdRng, range: [u64; 2]) -> u64 {
    let [min_b, max_b] = range;
    if max_b > min_b { rng.gen_range(min_b..=max_b) } else { min_b }
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
    fn name(&self) -> &'static str { "filesystem" }

    async fn generate(&self, recipe: &Recipe) -> Result<()> {
        let mut rng: StdRng = match recipe.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        for loc in &recipe.locations {
            std::fs::create_dir_all(&loc.path)
                .with_context(|| format!("create {:?}", loc.path))?;

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

                    let file = File::create(&fpath).with_context(|| format!("create {:?}", fpath))?;
                    file.set_len(size).with_context(|| format!("set_len {} for {:?}", size, fpath))?;
                    created_files.push(fpath);
                }
            }

            for _ in 0..duplicates_target {
                if created_files.is_empty() { break; }
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
        }

        Ok(())
    }
}
