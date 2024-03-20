#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub mod actors;
pub mod jobs;
pub mod tasks;

use tasks::indexer::{IndexerError, NonCriticalIndexerError};

#[derive(Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Indexer(#[from] IndexerError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::Indexer(e) => e.into(),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalJobError {
	// TODO: Add variants as needed
	#[error(transparent)]
	Indexer(#[from] NonCriticalIndexerError),
}

pub enum ProgressUpdate {
	TaskCount(usize),
	CompletedTaskCount(usize),
	Message(String),
	Phase(String),
}

pub trait ProgressReporter: Send + Sync + fmt::Debug + 'static {
	fn progress(&self, updates: Vec<ProgressUpdate>);

	fn progress_msg(&self, msg: impl Into<String>) {
		self.progress(vec![ProgressUpdate::Message(msg.into())]);
	}
}
