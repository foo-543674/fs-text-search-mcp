use rmcp::{model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo}, tool, ServerHandler};

use super::error::ServerError;

#[derive(Debug, Clone)]
pub struct SearchServer {
}

#[tool(tool_box)]
impl SearchServer {
    pub fn new() -> Self {
        SearchServer {}
    }

    #[tool(description = "Search for a string in a file")]
    async fn read_file() -> Result<String, ServerError> {
      Ok("Hello, World!".to_string())
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
        .build(),
      server_info: Implementation::from_build_env(),
      instructions: Some("This is a search server that can search for strings in files.".to_string()),
    }
  }
}