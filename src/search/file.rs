use std::{fmt::Debug, io};

pub struct File {
  pub path: String,
  pub content: String,
}

pub trait FileLoader: Send + Sync + Debug {
  fn load_directory(&self, path: &str) -> Box<dyn Iterator<Item = io::Result<File>>>;
}
