use std::path::Path;

use crate::search::file::FileFilter;

pub struct ExtensionFileFilter {
  allowed_extensions: Vec<String>,
}

impl ExtensionFileFilter {
  pub fn new(extensions: Vec<String>) -> Self {
    Self {
      allowed_extensions: extensions,
    }
  }
}

impl FileFilter for ExtensionFileFilter {
  fn is_target(&self, path: &str) -> bool {
    if let Some(ext) = Path::new(path).extension() {
      if let Some(ext_str) = ext.to_str() {
        return self.allowed_extensions.contains(&ext_str.to_string());
      }
    }
    false
  }
}
