use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileView {
  pub abs_path: String,
  pub name: String,
  pub created_at: u64,
  pub mod_at: u64,
  pub size: u64,
  pub is_dir: bool,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
  pub file_view: Vec<FileView>,
  pub tokenized: String,
}
