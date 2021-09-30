pub(crate) trait ToVec<T> {
  fn to_vec(self) -> Vec<T>;
}

pub(crate) type QueryMapper<T> = fn(&rusqlite::Row) -> rusqlite::Result<T>;

impl<T> ToVec<T> for rusqlite::MappedRows<'_, QueryMapper<T>> {
  fn to_vec(self) -> Vec<T> {
    let mut list = Vec::new();
    for row in self {
      list.push(row.unwrap());
    }
    list
  }
}
