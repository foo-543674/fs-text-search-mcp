use std::sync::Arc;

use fs_text_search_mcp::{
  file::{file_filter::ExtensionFileFilter, lazy_file_loader::LazyFileLoader},
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
  let file_loader = Arc::new(LazyFileLoader::new(file_filter));
  let service = SearchServer::new("foo".to_string(), file_loader)
    .serve(stdio())
    .await
    .inspect_err(|e| {
      tracing::error!("serving error: {:?}", e);
    })?;

  service.waiting().await?;
  Ok(())
}
