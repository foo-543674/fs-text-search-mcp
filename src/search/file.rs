use std::io;

pub struct File {
  pub path: String,
  pub content: String,
}

pub trait FileLoader {
  fn load_directory(&self, path: &str) -> Box<dyn Iterator<Item = io::Result<File>>>;
}
