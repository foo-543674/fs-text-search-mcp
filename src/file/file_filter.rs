use std::path::Path;

pub trait FileFilter {
  fn is_target(&self, path: &Path) -> bool;
}

pub struct ExtensionFileFilter {
    allowed_extensions: Vec<&'static str>,
}

impl ExtensionFileFilter {
    pub fn new(extensions: Vec<&'static str>) -> Self {
        Self {
            allowed_extensions: extensions,
        }
    }
}

impl FileFilter for ExtensionFileFilter {
    fn is_target(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return self.allowed_extensions.contains(&ext_str);
            }
        }
        false
    }
}
