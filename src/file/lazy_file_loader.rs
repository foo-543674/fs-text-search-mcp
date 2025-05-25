use std::{fs, path::Path};

use crate::search::file::{File, FileLoader};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct LazyFileLoader {}

impl LazyFileLoader {
  pub fn new() -> Self {
    LazyFileLoader {}
  }
}

impl FileLoader for LazyFileLoader {
  fn load_directory(
    &self,
    path: &str,
  ) -> Box<dyn Iterator<Item = std::io::Result<crate::search::file::File>>> {
    let walker = WalkDir::new(path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.path().is_file())
      .filter(|e| is_text_file(e.path()));

    Box::new(walker.map(|entry| {
      let path = entry.path().to_path_buf();
      fs::read_to_string(&path).map(|content| File {
        path: path.to_string_lossy().to_string(),
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
