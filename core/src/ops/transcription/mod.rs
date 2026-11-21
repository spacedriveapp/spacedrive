//! # Transcription
//!
//! Turns the speech in any audio or video file into a timestamped transcript
//! and feeds it to the content text index, so `search.content` and the agent
//! MCP `search_content` tool find spoken words and answer with the second
//! they were said.
//!
//! The job runs Whisper on the device that owns the source. It reads the
//! file's speech audio sidecar (`audio/16k_mono.wav`) when one exists and
//! otherwise decodes the original with FFmpeg, in two-minute chunks cut at
//! the quietest moment near each boundary, so a long recording never sits in
//! memory whole. Within a chunk, only the stretches with voice go to the model,
//! each trimmed of the silence around it: the model would invent words for
//! silence, and it places an input's first words near its start, so trimming
//! is what keeps each timestamp on the second the words were said.
//!
//! Each transcript is written as three content-addressed sidecars beside the
//! file's other media sidecars: `transcript/default.json` with timed
//! segments, and `default.srt` and `default.vtt` for editors and players.
//! The JSON is the source of truth: a later pass over the same bytes reads it
//! back instead of running the model again.
//!
//! `transcript` holds the sidecar format and is always compiled, so readers
//! need no speech engine. Everything that runs Whisper is behind the
//! `speech-to-text` cargo feature, which is off by default.
//!
//! ## Example
//! ```json
//! { "path": "/Users/me/Footage", "model": "base", "language": null, "force": false }
//! ```
//! sent as the `media.transcribe` library action queues one job over every
//! audio and video file under the path.

pub mod transcript;

#[cfg(feature = "speech-to-text")]
pub mod action;
#[cfg(feature = "speech-to-text")]
pub mod audio;
#[cfg(feature = "speech-to-text")]
mod engine;
#[cfg(feature = "speech-to-text")]
pub mod job;
