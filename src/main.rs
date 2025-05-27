use std::sync::{Arc, Mutex};

use fs_text_search_mcp::{
  file::{
    file_filter::ExtensionFileFilter, file_watcher::{FileWatcher, NotifyFileWatcher},
    lazy_file_loader::LazyFileLoader,
  },
  search::{index_operation::IndexUpdateQueue, text_index::TextIndex},
  servers::search::SearchServer,
};
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
    .with_writer(std::io::stderr)
    .with_ansi(false)
    .init();

  tracing::info!("Starting Server...");

  let file_filter = Arc::new(ExtensionFileFilter::new(vec!["txt", "md"]));
  let file_loader = Arc::new(LazyFileLoader::new(file_filter.clone()));

  let index = Arc::new(Mutex::new(TextIndex::new()?));
  {
    let mut idx = index.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
    idx.initialize_index(file_loader.clone().as_ref(), "./foo")?;
  }

  let service = SearchServer::new(index.clone())
    .serve(stdio())
    .await
    .inspect_err(|e| {
      tracing::error!("serving error: {:?}", e);
    })?;

  let mut file_watcher = NotifyFileWatcher::new(file_filter.clone());
  let index_operation = Arc::new(IndexUpdateQueue::new(index.clone()));

  file_watcher.watch_directory(
    "./foo",
    {
      let index_operation = index_operation.clone();
      Box::new(move |file| {
        tracing::info!("File created: {}", file.path);
        index_operation.queue_create(file.clone());
      })
    },
    {
      let index_operation = index_operation.clone();
      Box::new(move |file| {
        tracing::info!("File modified: {}", file.path);
        index_operation.queue_modify(file.clone());
      })
    },
    {
      let index_operation = index_operation.clone();
      Box::new(move |path| {
        tracing::info!("File deleted: {}", path);
        index_operation.queue_delete(path.to_string());
      })
    },
  )?;

  service.waiting().await?;
  Ok(())
}
