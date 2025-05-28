use anyhow::Result;
use std::{
  sync::{Arc, Mutex, mpsc},
  thread,
  time::Duration,
};

use super::{
  file::{FileFilter, FileLoader, FileOperation},
  text_index::TextIndex,
};

const WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK: u64 = 500;

pub struct IndexOperation {
  index: Arc<Mutex<TextIndex>>,
  sender: mpsc::Sender<FileOperation>,
  _worker_handle: thread::JoinHandle<()>,
}

impl IndexOperation {
  pub fn new(
    text_index: Arc<Mutex<TextIndex>>,
    file_filter: Arc<dyn FileFilter + Send + Sync>,
    file_loader: Arc<dyn FileLoader + Send + Sync>,
  ) -> Result<Self> {
    let (sender, receiver) = mpsc::channel::<FileOperation>();

    let file_filter_clone = file_filter.clone();
    let file_loader_clone = file_loader.clone();
    let text_index_for_worker = text_index.clone();

    let worker_handle = thread::Builder::new()
      .name("index-update-worker".to_string())
      .spawn(move || {
        subscribe_operations(
          receiver,
          &process_operations(text_index_for_worker, file_filter_clone, file_loader_clone),
        )
      })
      .expect("Failed to spawn index update worker");

    Ok(IndexOperation {
      index: text_index,
      sender,
      _worker_handle: worker_handle,
    })
  }

  pub fn initialize_index(
    &self,
    target_dir: &str,
    file_filter: Arc<dyn FileFilter + Send + Sync>,
    file_loader: Arc<dyn FileLoader + Send + Sync>,
  ) -> Result<()> {
    let mut index = match self.index.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    };
    file_loader
      .load_directory(target_dir)
      .filter_map(Result::ok)
      .filter(|file| file_filter.is_target(&file.path))
      .try_for_each(|file| index.add_doc(&file))?;
    index.commit()
  }

  pub fn enqueue(&self, operation: &FileOperation) -> Result<()> {
    self
      .sender
      .send(operation.clone())
      .map_err(|e| anyhow::anyhow!("Failed to queue index operation: {}", e))
  }
}

fn subscribe_operations(
  receiver: mpsc::Receiver<FileOperation>,
  handler: &impl Fn(&Vec<FileOperation>) -> Result<()>,
) {
  fn receive_with_timeout(
    receiver: &mpsc::Receiver<FileOperation>,
    timeout: Option<Duration>,
  ) -> Result<FileOperation, mpsc::RecvTimeoutError> {
    match timeout {
      Some(t) => receiver.recv_timeout(t),
      None => receiver
        .recv()
        .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
    }
  }

  fn handle_operations(
    operations: &mut Vec<FileOperation>,
    handler: &impl Fn(&Vec<FileOperation>) -> Result<()>,
  ) {
    if let Err(e) = handler(operations) {
      tracing::error!("Failed to handle operations: {}", e);
    }
    operations.clear();
  }

  let mut operations = Vec::new();

  loop {
    let timeout = if operations.is_empty() {
      None
    } else {
      Some(Duration::from_millis(WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK))
    };

    match receive_with_timeout(&receiver, timeout) {
      Ok(operation) => {
        operations.push(operation);
        const MAX_BULK_OPERATION_SIZE: usize = 10;
        if operations.len() >= MAX_BULK_OPERATION_SIZE {
          handle_operations(&mut operations, handler);
        }
      }
      Err(mpsc::RecvTimeoutError::Timeout) => {
        if !operations.is_empty() {
          handle_operations(&mut operations, handler);
        }
      }
      Err(mpsc::RecvTimeoutError::Disconnected) => {
        if !operations.is_empty() {
          handle_operations(&mut operations, handler);
        }
        tracing::info!("Index update worker shutting down");
        break;
      }
    }
  }
}

fn process_operations(
  text_index: Arc<Mutex<TextIndex>>,
  file_filter: Arc<dyn FileFilter>,
  file_loader: Arc<dyn FileLoader>,
) -> impl Fn(&Vec<FileOperation>) -> Result<()> {
  move |operations| {
    if let Ok(mut index) = text_index.lock() {
      for op in operations {
        match op {
          FileOperation::FileCreated(path) => {
            if file_filter.is_target(path) {
              let file = file_loader.load_file(path)?;
              index.add_doc(&file)?;
            }
          }
          FileOperation::FileModified(path) => {
            if file_filter.is_target(path) {
              let file = file_loader.load_file(path)?;
              index.replace_doc(&file)?;
            }
          }
          FileOperation::FileDeleted(path) => {
            index.delete_doc(path)?;
          }
          FileOperation::FileRenamed { old_path, new_path } => {
            match (
              file_filter.is_target(old_path),
              file_filter.is_target(new_path),
            ) {
              (true, true) => {
                let file = file_loader.load_file(new_path)?;
                index.delete_doc(old_path)?;
                index.add_doc(&file)?;
              }
              (true, false) => {
                index.delete_doc(old_path)?;
              }
              (false, true) => {
                let file = file_loader.load_file(new_path)?;
                index.add_doc(&file)?;
              }
              (false, false) => {}
            }
          }
          FileOperation::DirectoryDeleted(path) => {
            index.delete_docs_by_path_prefix(path)?;
          }
          FileOperation::DirectoryRenamed { old_path, new_path } => {
            index.delete_docs_by_path_prefix(old_path)?;
            file_loader
              .load_directory(new_path)
              .filter_map(Result::ok)
              .filter(|file| file_filter.is_target(&file.path))
              .for_each(|file| {
                if let Err(e) = index.add_doc(&file) {
                  tracing::error!("Failed to add document after directory rename: {}", e);
                }
              });
          }
        }
      }
      index.commit()?;
    } else {
      tracing::error!("Failed to lock text index for processing operations");
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::search::file::{File, FileFilter, FileLoader};

  struct MockFileFilter;

  const ALLOWED_EXTENSION: &str = ".txt";

  impl FileFilter for MockFileFilter {
    fn is_target(&self, path: &str) -> bool {
      path.ends_with(ALLOWED_EXTENSION)
    }
  }

  struct MockFileLoader {
    files: Vec<File>,
    loaded_file_content: String,
  }

  impl MockFileLoader {
    pub fn new(files: Vec<File>, loaded_file_content: String) -> Self {
      MockFileLoader {
        files,
        loaded_file_content,
      }
    }
  }
  impl FileLoader for MockFileLoader {
    fn load_directory(&self, _path: &str) -> Box<dyn Iterator<Item = Result<File>> + '_> {
      Box::new(self.files.iter().cloned().map(Ok))
    }

    fn load_file(&self, path: &str) -> Result<File> {
      Ok(File::new(
        path.to_string(),
        self.loaded_file_content.clone(),
      ))
    }
  }

  fn create_initialize_file_loader() -> Arc<dyn FileLoader + Send + Sync> {
    Arc::new(MockFileLoader::new(
      vec![
        File::new(
          "add_at_initialize1.txt".to_string(),
          "Must find content 1".to_string(),
        ),
        File::new(
          "add_at_initialize2.txt".to_string(),
          "Must find content 2".to_string(),
        ),
        File::new(
          "ignore_at_initialize1.md".to_string(),
          "Must not find content 3".to_string(),
        ),
        File::new(
          "/indir/add_at_initialize3.txt".to_string(),
          "Must find content 4".to_string(),
        ),
        File::new(
          "/indir/add_at_initialize4.txt".to_string(),
          "Must find content 5".to_string(),
        ),
        File::new(
          "/indir/ignore_at_initialize2.md".to_string(),
          "Must not find content 6".to_string(),
        ),
      ],
      "Loaded content.".to_string(),
    ))
  }

  #[test]
  fn index_operation_should_initialize_index_with_files_in_directory() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");
    let index = text_index.lock().unwrap();
    let results = index.search("content").expect("Failed to search index");
    assert_eq!(results.len(), 4);
    assert!(results.iter().any(|r| r.contains("add_at_initialize1.txt")));
    assert!(results.iter().any(|r| r.contains("add_at_initialize2.txt")));
    assert!(
      results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize3.txt"))
    );
    assert!(
      results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize4.txt"))
    );
    assert!(
      !results
        .iter()
        .any(|r| r.contains("ignore_at_initialize1.md"))
    );
    assert!(
      !results
        .iter()
        .any(|r| r.contains("/indir/ignore_at_initialize2.md"))
    );
  }

  #[test]
  fn index_operation_should_update_index_on_file_created() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::FileCreated("added.txt".to_string()))
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("Loaded").expect("Failed to search index");
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("added.txt"));
  }

  #[test]
  fn index_operation_should_update_index_on_file_modified() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::FileModified("modified.txt".to_string()))
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("Loaded").expect("Failed to search index");
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("modified.txt"));
  }

  #[test]
  fn index_operation_should_update_index_on_file_deleted() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::FileDeleted(
        "add_at_initialize1.txt".to_string(),
      ))
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("content").expect("Failed to search index");
    assert_eq!(results.len(), 3);
    assert!(!results.iter().any(|r| r.contains("add_at_initialize1.txt")));
  }

  #[test]
  fn index_operation_should_update_index_on_file_renamed() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::FileRenamed {
        old_path: "add_at_initialize1.txt".to_string(),
        new_path: "renamed.txt".to_string(),
      })
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("Loaded").expect("Failed to search index");
    assert_eq!(results.len(), 1);
    assert!(results[0].contains("renamed.txt"));
    let results = index.search("content").expect("Failed to search index");
    assert_eq!(results.len(), 4);
    assert!(!results.iter().any(|r| r.contains("add_at_initialize1.txt")));
  }

  #[test]
  fn index_operation_should_update_index_on_directory_deleted() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let index_operation =
      IndexOperation::new(text_index.clone(), file_filter.clone(), file_loader.clone())
        .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::DirectoryDeleted("/indir".to_string()))
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("content").expect("Failed to search index");
    assert_eq!(results.len(), 2);
    assert!(
      !results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize3.txt"))
    );
    assert!(
      !results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize4.txt"))
    );
  }

  #[test]
  fn index_operation_should_update_index_on_directory_renamed() {
    let text_index = Arc::new(Mutex::new(TextIndex::new().unwrap()));
    let file_filter = Arc::new(MockFileFilter);
    let file_loader = create_initialize_file_loader();

    let file_loader_for_rename = Arc::new(MockFileLoader::new(
      vec![
        File::new(
          "/renamed_dir/add_at_initialize3.txt".to_string(),
          "Must find content 4".to_string(),
        ),
        File::new(
          "/renamed_dir/add_at_initialize4.txt".to_string(),
          "Must find content 5".to_string(),
        ),
      ],
      "Loaded content after rename.".to_string(),
    ));

    let index_operation = IndexOperation::new(
      text_index.clone(),
      file_filter.clone(),
      file_loader_for_rename.clone(),
    )
    .expect("Failed to create IndexOperation");
    index_operation
      .initialize_index("test_dir", file_filter.clone(), file_loader.clone())
      .expect("Failed to initialize index");

    index_operation
      .enqueue(&FileOperation::DirectoryRenamed {
        old_path: "/indir".to_string(),
        new_path: "/renamed_dir".to_string(),
      })
      .expect("Failed to enqueue operation");

    thread::sleep(Duration::from_millis(
      WAIT_MILLIS_FOR_NEXT_UPDATE_TO_BULK * 2,
    ));

    let index = text_index.lock().unwrap();
    let results = index.search("content").expect("Failed to search index");
    assert_eq!(results.len(), 4);
    assert!(
      !results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize3.txt"))
    );
    assert!(
      !results
        .iter()
        .any(|r| r.contains("/indir/add_at_initialize4.txt"))
    );
    assert!(
      results
        .iter()
        .any(|r| r.contains("/renamed_dir/add_at_initialize3.txt"))
    );
    assert!(
      results
        .iter()
        .any(|r| r.contains("/renamed_dir/add_at_initialize4.txt"))
    );
  }
}
