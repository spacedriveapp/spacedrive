use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub seed: Option<u64>,
    pub locations: Vec<RecipeLocation>,
    #[serde(default)]
    pub media: Option<RecipeMedia>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeLocation {
    pub path: PathBuf,
    pub structure: Structure,
    pub files: FileSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub depth: usize,
    pub fanout_per_dir: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSpec {
    pub total: usize,
    pub size_buckets: HashMap<String, SizeBucket>,
    #[serde(default)]
    pub duplicate_ratio: Option<f32>,
    #[serde(default)]
    pub media_ratio: Option<f32>,
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeBucket {
    pub range: [u64; 2],
    pub share: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeMedia {
    #[serde(default)]
    pub generate_thumbnails: Option<bool>,
    #[serde(default)]
    pub synthetic_video: Option<SynthVideo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthVideo {
    pub enabled: bool,
    pub duration_s: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
