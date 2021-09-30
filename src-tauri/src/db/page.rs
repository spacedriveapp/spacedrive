use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PageRequest {
  #[serde(default)]
  pub(crate) page: i64,
  #[serde(default)]
  pub(crate) size: i64,
  #[serde(default)]
  pub(crate) sort_by: String,
  #[serde(default)]
  pub(crate) direction: SortDirection,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum SortDirection {
  ASC,
  DESC,
}

impl Default for SortDirection {
  fn default() -> Self {
    SortDirection::ASC
  }
}

impl Display for SortDirection {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PageResponse<T> {
  content: Vec<T>,
  total_pages: i64,
  total_elements: i64,
}

impl<T> PageResponse<T> {
  pub(crate) fn new(content: Vec<T>, total_pages: i64, total_elements: i64) -> Self {
    PageResponse {
      content,
      total_pages,
      total_elements,
    }
  }
}
