use crate::db::entity::file;
use serde::Serialize;

#[derive(Serialize)]
pub struct Directory {
  pub directory: file::Model,
  pub contents: Vec<file::Model>,
}
