use anyhow::Result;

#[derive(Debug, Clone)]
pub struct File {
  pub path: String,
  pub content: String,
}

impl File {
  pub fn new(path: String, content: String) -> Self {
    Self { path, content }
  }
}

pub trait FileFilter {
  fn is_target(&self, path: &str) -> bool;
}

pub trait FileLoader {
  fn load_directory(&self, path: &str) -> Box<dyn Iterator<Item = Result<File>> + '_>;
  fn load_file(&self, path: &str) -> Result<File>;
}

#[derive(Debug, Clone)]
pub enum FileOperation {
  FileCreated(String),
  FileModified(String),
  FileRenamed { old_path: String, new_path: String },
  DirectoryRenamed { old_path: String, new_path: String },
  FileDeleted(String),
  DirectoryDeleted(String),
}

pub type FileOperationHandler = dyn Fn(&FileOperation) -> Result<()> + Send + Sync;

pub trait FileWatcher {
  fn watch_directory(&mut self, path: &str, handler: Box<FileOperationHandler>) -> Result<()>;
  fn stop_watching(&mut self) -> Result<()>;
}
