use sea_orm::DbErr;
use snafu::Snafu;
use std::{io, path::PathBuf};

#[derive(Debug, Snafu)]
pub enum Error {
  #[snafu(display("Unable to read configuration from {}: {}", path, source))]
  DatabaseConnectionResult { source: DbErr, path: String },

  #[snafu(display("Unable to write result to {}: {}", path.display(), source))]
  WriteResult { source: io::Error, path: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
