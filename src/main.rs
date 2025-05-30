use clap::Parser;
use fs_text_search_mcp::application;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
  /// Directory to watch for file changes
  #[arg(short, long, default_value = ".")]
  watch_dir: PathBuf,

  /// Directory to store the search index (if not specified, use in-memory)
  #[arg(short, long)]
  index_dir: Option<PathBuf>,

  /// File extensions to include (comma-separated)
  #[arg(short, long, default_value = "txt,md")]
  extensions: String,

  /// Enable verbose logging (debug level)
  #[arg(short, long)]
  verbose: bool,

  // Only error logging (ideal for MCP usage)
  #[arg(short, long)]
  quiet: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();

  let log_level = if cli.verbose {
    tracing::Level::DEBUG
  } else if cli.quiet {
    tracing::Level::ERROR
  } else {
    tracing::Level::INFO
  };

  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive(log_level.into()))
    .with_writer(std::io::stderr)
    .with_ansi(false)
    .init();

  let application = application::Application::new(cli.watch_dir, cli.index_dir, cli.extensions)?;
  application.run().await?;

  Ok(())
}
