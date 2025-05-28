use anyhow::Result;
use walkdir::WalkDir;

use super::read_file::path_to_file;
use crate::search::file::{File, FileLoader};

pub struct LazyFileLoader {}

impl LazyFileLoader {
  pub fn new() -> Self {
    LazyFileLoader {}
  }
}

impl Default for LazyFileLoader {
  fn default() -> Self {
    Self::new()
  }
}

impl FileLoader for LazyFileLoader {
  fn load_directory(
    &self,
    dir_path: &str,
  ) -> Box<dyn Iterator<Item = Result<crate::search::file::File>> + '_> {
    let paths = WalkDir::new(dir_path)
      .into_iter()
      .flatten()
      .map(|e| e.path().to_owned())
      .flat_map(|p| p.canonicalize())
      .filter(|p| p.is_file());

    Box::new(paths.map(|p| path_to_file(&p)))
  }

  fn load_file(&self, path: &str) -> Result<File> {
    let file_path = std::path::Path::new(path);
    path_to_file(file_path)
  }
}
