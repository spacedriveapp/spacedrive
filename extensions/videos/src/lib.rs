//! # Videos
//!
//! `com.spacedrive.videos` turns a folder of footage into shots you can
//! browse by person, place and tag. The core does the media work an
//! extension cannot do from WASM: it cuts the proxy, transcribes the audio
//! and splits the video into shots with one keyframe each. This extension
//! reads those sidecars, asks `ai.infer` to caption each keyframe, label its
//! place and find its faces, and writes the answers as its own `scene` and
//! `faces` sidecars. Faces are clustered into a people roster that you name
//! and correct; your names and corrections are kept as an edit log, so a
//! re-cluster replays them instead of losing them.
//!
//! Every step degrades: a file without a shot list waits for the core, and
//! a category without a provider is reported `not_available` once and
//! retried on the next run.

mod analyze;
mod infer;
mod models;
mod people;
mod shots;

use spacedrive_sdk::extension;

use analyze::*;
pub use models::{Footage, Person, RosterEdit};
use people::*;

#[extension(
	id = "com.spacedrive.videos",
	name = "Videos",
	version = "0.1.0",
	jobs = [analyze_footage, cluster_people, edit_roster],
	models = [Footage, Person, RosterEdit],
)]
struct Videos;
