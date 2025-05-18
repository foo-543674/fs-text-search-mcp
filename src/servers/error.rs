use rmcp::model::{Content, IntoContents};

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ServerError(anyhow::Error);

impl IntoContents for ServerError {
    fn into_contents(self) -> Vec<Content> {
      Content::text(format!("Error: {}", self))
        .into_contents()
    }
}