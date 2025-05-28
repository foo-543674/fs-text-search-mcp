# MCP Server for Full Text Search in local file

A Model Context Protocol (MCP) server that provides full-text search capabilities for local files. It watches directories for file changes and maintains a search index using Tantivy.

## Features

- 🔍 **Full-text search** with Tantivy search engine
- 📁 **Directory watching** with automatic index updates
- 🔄 **Real-time synchronization** when files are created, modified, or deleted
- 💾 **Persistent index** option (file-based or in-memory)
- 🎯 **Configurable extensions** to filter file types
- ⚡ **Debounced file events** to avoid duplicate processing
- 🛠️ **CLI configuration** with command-line options

## Usage

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd fs-text-search-mcp

# Build the project
cargo build --release
```

### Basic Usage

```bash
# Basic usage with in-memory index
./target/release/fs-text-search-mcp --watch-dir ./documents

# With file-based persistent index
./target/release/fs-text-search-mcp \
  --watch-dir ./documents \
  --index-dir ./search_index

# Specify file extensions to monitor
./target/release/fs-text-search-mcp \
  --watch-dir ./documents \
  --extensions "txt,md,rs,py,js,ts,json"

# Enable verbose logging
./target/release/fs-text-search-mcp \
  --watch-dir ./documents \
  --verbose
```

### Command Line Options

| Option | Short | Description | Default |
|--------|-------|-------------|--------|
| `--watch-dir` | `-w` | Directory to watch for file changes | `./target_dir` |
| `--index-dir` | `-i` | Directory to store search index (optional) | In-memory |
| `--extensions` | `-e` | File extensions to include (comma-separated) | `txt,md` |
| `--verbose` | `-v` | Enable verbose logging | false |
| `--help` | `-h` | Show help message | - |

### Examples

#### Monitor a project directory
```bash
./fs-text-search-mcp \
  --watch-dir ~/projects/my-project \
  --index-dir ~/.cache/search-index \
  --extensions "rs,toml,md,txt" \
  --verbose
```

#### Monitor documentation
```bash
./fs-text-search-mcp \
  --watch-dir ~/Documents \
  --extensions "md,txt,pdf,docx" \
  --index-dir ./docs-index
```

#### Development mode
```bash
cargo run -- \
  --watch-dir ./src \
  --extensions "rs,toml" \
  --verbose
```

### MCP Client Integration

Once the server is running, you can interact with it via MCP protocol:

#### Available Tools

- **search_index**: Search for text within indexed files
  - Parameter: `keyword` (string) - The search query

#### Example MCP Interactions

```json
// Initialize the connection
{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}

// Get available tools
{"jsonrpc":"2.0","method":"tools/list","id":1}

// Search for content
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_index","arguments":{"keyword":"function"}}}

// Search with multiple keywords
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_index","arguments":{"keyword":"async function"}}}
```

## Development

### Local Development

```bash
# Run in development mode
cargo run -- --watch-dir ./target_dir --verbose

# Run tests
cargo test

# Check code formatting
cargo fmt

# Run clippy for linting
cargo clippy
```

### Manual Testing with MCP Protocol

You can test the MCP server manually using JSON-RPC messages:

```bash
# Start the server
cargo run -- --watch-dir ./test-files --verbose

# In another terminal, send MCP messages:
# Initialize request
echo '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}' | nc localhost 3000

# Notify initialized
echo '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' | nc localhost 3000

# Get tools
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | nc localhost 3000

# Search
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_index","arguments":{"keyword":"function"}}}' | nc localhost 3000
```

### Architecture

The project follows a modular architecture with clear separation of concerns:

```
src/
├── file/                    # File system operations
│   ├── file_filter.rs      # File filtering logic
│   ├── file_watcher.rs     # File system monitoring
│   ├── lazy_file_loader.rs # Directory scanning
│   └── read_file.rs        # File reading utilities
├── search/                  # Search functionality
│   ├── file.rs             # File model and traits
│   ├── index_operation.rs  # Index update queue
│   └── text_index.rs       # Tantivy index wrapper
├── servers/                 # MCP server implementation
│   ├── error.rs            # Error handling
│   └── search.rs           # Search server
└── main.rs                  # Application entry point
```

### Dependencies

- **tantivy**: Full-text search engine
- **notify-debouncer-full**: File system event debouncing
- **rmcp**: Model Context Protocol implementation
- **clap**: Command-line argument parsing
- **tokio**: Async runtime
- **tracing**: Structured logging