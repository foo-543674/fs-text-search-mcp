use std::{fs, sync::Arc};

use crate::search::file::{File, FileLoader};
use walkdir::WalkDir;

use super::file_filter::FileFilter;

pub struct LazyFileLoader {
  filter: Arc<dyn FileFilter>,
}

impl LazyFileLoader {
  pub fn new(filter: Arc<dyn FileFilter>) -> Self {
    LazyFileLoader {
      filter: filter.clone(),
    }
  }
}

impl FileLoader for LazyFileLoader {
  fn load_directory(
    &self,
    dir_path: &str,
  ) -> Box<dyn Iterator<Item = std::io::Result<crate::search::file::File>> + '_> {
    let entries = WalkDir::new(dir_path)
      .into_iter()
      .flatten()
      .filter(|e| e.path().is_file())
      .filter(|e| self.filter.is_target(e.path()));

    Box::new(entries.map(|entry| {
      let file_path = entry.path().to_path_buf();
      fs::read_to_string(&file_path).map(|content| File {
        path: file_path.to_string_lossy().to_string(),
        content,
      })
    }))
  }
}
