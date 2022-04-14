// PathContext provides the indexer with instruction to handle particular directory structures and identify rich context.
pub struct PathContext {
  // an app specific key "com.github.repo"
  pub key: String,
  pub name: String,
  pub is_dir: bool,
  // possible file extensions for this path
  pub extensions: Vec<String>,
  // sub-paths that must be found
  pub must_contain_sub_paths: Vec<String>,
  // sub-paths that are ignored
  pub always_ignored_sub_paths: Option<String>,
}
