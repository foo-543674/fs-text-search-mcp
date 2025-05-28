use anyhow::Result;
use rmcp::{ServiceExt, service::QuitReason, transport::stdio};
use std::{
  path::PathBuf,
  sync::{Arc, Mutex},
};

use crate::{
  file::{
    file_filter::ExtensionFileFilter, file_watcher::NotifyFileWatcher,
    lazy_file_loader::LazyFileLoader,
  },
  search::{file::{FileLoader, FileWatcher}, index_operation::IndexOperation, text_index::TextIndex},
  servers::search::SearchServer,
};

pub struct Application {
  index: Arc<Mutex<TextIndex>>,
  file_loader: Arc<dyn FileLoader + Send + Sync>,
  _index_operation: Arc<IndexOperation>,
  _file_watcher: NotifyFileWatcher,
}

impl Application {
  pub fn new(watch_dir: PathBuf, index_dir: Option<PathBuf>, extensions: String) -> Result<Self> {
    let index = if let Some(index_dir) = &index_dir {
      Arc::new(Mutex::new(TextIndex::new_with_directory(index_dir)?))
    } else {
      Arc::new(Mutex::new(TextIndex::new()?))
    };
    let file_filter = Arc::new(ExtensionFileFilter::new(
      extensions
        .split(",")
        .map(|e| e.to_string())
        .collect::<Vec<_>>(),
    ));
    let file_loader = Arc::new(LazyFileLoader::new());
    let mut file_watcher = NotifyFileWatcher::new();

    let index_operation = Arc::new(IndexOperation::new(
      index.clone(),
      file_filter.clone(),
      file_loader.clone(),
    )?);
    index_operation.initialize_index(
      watch_dir.to_string_lossy().as_ref(),
      file_filter.clone(),
      file_loader.clone(),
    )?;

    file_watcher.watch_directory(watch_dir.to_string_lossy().as_ref(), {
      let index_operation = index_operation.clone();
      Box::new(move |op| index_operation.enqueue(op))
    })?;

    return Ok(Application {
      index,
      file_loader: file_loader.clone(),
      _index_operation: index_operation,
      _file_watcher: file_watcher,
    });
  }

  pub async fn run(&self) -> Result<QuitReason> {
    let service = SearchServer::new(self.index.clone(), self.file_loader.clone())
      .serve(stdio())
      .await
      .inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
      })?;
    service.waiting().await.map_err(|e| e.into())
  }
}
