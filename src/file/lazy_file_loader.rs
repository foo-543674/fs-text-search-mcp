use std::{fs, path::Path};

use crate::search::file::{File, FileLoader};
use walkdir::WalkDir;

pub struct LazyFileLoader {}

impl LazyFileLoader {
  pub fn new() -> Self {
    LazyFileLoader {}
  }
}

impl FileLoader for LazyFileLoader {
  fn load_directory(
    &self,
    dir_path: &str,
  ) -> Box<dyn Iterator<Item = std::io::Result<crate::search::file::File>>> {
    let entries = WalkDir::new(dir_path)
      .into_iter()
      .flatten()
      .filter(|e| e.path().is_file())
      .filter(|e| is_text_file(e.path()));

    Box::new(entries.map(|entry| {
      let file_path = entry.path().to_path_buf();
      fs::read_to_string(&file_path).map(|content| File {
        path: file_path.to_string_lossy().to_string(),
        content,
      })
    }))
  }
}

fn is_text_file(path: &Path) -> bool {
  if let Some(ext) = path.extension() {
    matches!(ext.to_str(), Some("txt" | "md" | "rs" | "toml" | "json"))
  } else {
    false
  }
}
