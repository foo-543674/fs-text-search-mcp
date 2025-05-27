use anyhow::Result;
use std::{path::Path, thread, time::Duration};

use crate::search::file::File;

pub fn read_file_with_retry(path: &Path, max_retries: u32) -> Result<String> {
  let mut last_error = None;

  for attempt in 0..=max_retries {
    match std::fs::read_to_string(path) {
      Ok(content) => return Ok(content),
      Err(e) => {
        last_error = Some(e);
        if attempt < max_retries {
          thread::sleep(Duration::from_millis(10 * (attempt + 1) as u64));
        }
      }
    }
  }

  Err(anyhow::anyhow!(
    "Failed to read file after {} attempts: {}",
    max_retries + 1,
    last_error.unwrap()
  ))
}

pub fn path_to_file(path: &Path) -> Result<File> {
  let content = read_file_with_retry(path, 3)?;
  Ok(File {
    path: path.to_string_lossy().to_string(),
    content,
  })
}
