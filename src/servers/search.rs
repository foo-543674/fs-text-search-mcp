use std::sync::Arc;

use rmcp::{
  ServerHandler,
  model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
  schemars,
  schemars::JsonSchema,
  tool,
};

use crate::search::{file::FileLoader, search_in_dir::search_in_dir};

use super::error::ServerError;

#[derive(Debug, Clone)]
pub struct SearchServer {
  root_path: String,
  file_loader: Arc<dyn FileLoader>,
}

#[derive(JsonSchema, Debug, serde::Deserialize)]
pub struct SearchParams {
  #[schemars(description = "Keyword to search for. Use space to separate multiple keywords.")]
  pub keyword: String,
}

#[tool(tool_box)]
impl SearchServer {
  pub fn new(root_path: String, file_loader: Arc<dyn FileLoader>) -> Self {
    SearchServer {
      root_path,
      file_loader,
    }
  }

  #[tool(description = "Search for a string in a file")]
  async fn search_index(&self, #[tool(aggr)] params: SearchParams) -> Result<String, ServerError> {
    search_in_dir(self.file_loader.as_ref(), &self.root_path)(&params.keyword)
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
