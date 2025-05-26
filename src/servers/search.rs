use std::{fmt::Debug, sync::{Arc, Mutex}};

use rmcp::{
  ServerHandler,
  model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
  schemars,
  schemars::JsonSchema,
  tool,
};

use crate::search::{file::FileLoader, text_index::TextIndex};

use super::error::ServerError;

#[derive(Clone)]
pub struct SearchServer {
  root_path: String,
  index: Arc<Mutex<TextIndex>>,
}

impl Debug for SearchServer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("SearchServer")
      .field("root_path", &self.root_path)
      .finish()
  }
}

#[derive(JsonSchema, Debug, serde::Deserialize)]
pub struct SearchParams {
  #[schemars(description = "Keyword to search for. Use space to separate multiple keywords.")]
  pub keyword: String,
}

#[tool(tool_box)]
impl SearchServer {
  pub fn new(root_path: String, file_loader: Arc<dyn FileLoader>) -> Self {
    let mut index = TextIndex::new().expect("Failed to create text index");
    index
      .initialize_index(file_loader.as_ref(), &root_path)
      .expect("Failed to initialize index");

    SearchServer {
      root_path,
      index: Arc::new(Mutex::new(index)),
    }
  }

  #[tool(description = "Search for a string in a file")]
  async fn search_index(&self, #[tool(aggr)] params: SearchParams) -> Result<String, ServerError> {
    let index = self.index.lock().map_err(|_| ServerError(anyhow::anyhow!("Failed to lock index")))?;
    index
      .search(&params.keyword)
      .map_err(ServerError)
      .and_then(|results| {
        if results.is_empty() {
          Err(ServerError(anyhow::anyhow!("No results found.")))
        } else {
          Ok(format!("[{}]", results.join(", ")))
        }
      })
  }
}

#[tool(tool_box)]
impl ServerHandler for SearchServer {
  fn get_info(&self) -> ServerInfo {
    ServerInfo {
      protocol_version: ProtocolVersion::V_2024_11_05,
      capabilities: ServerCapabilities::builder()
        .enable_prompts()
        .enable_resources()
        .enable_tools()
        .build(),
      server_info: Implementation::from_build_env(),
      instructions: Some(
        "This is a search server that can search for strings in files.".to_string(),
      ),
    }
  }
}
