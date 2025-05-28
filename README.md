# MCP Server for Full Text Search in local file

A Model Context Protocol (MCP) server that provides full-text search capabilities for local files. It watches directories for file changes and maintains a search index using Tantivy.

## Features

- 🔍 **Full-text search** with Tantivy search engine
- 🔄 **Real-time synchronization** when files are created, modified, or deleted

## Usage

### MCP Client Configuration

Add the following configuration to your Claude Desktop config file:

#### macOS Configuration

**Config file location**: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "fs-text-search": {
      "command": "docker",
      "args": [
        "run", "--rm", "-i",
        "-v", "/path/to/your/documents:/home/source",
        "-v", "/path/to/search/index:/home/index",
        "foobarxyzzy/fs-text-search-mcp",
        "--extensions", "txt,md,rs,py,js,ts,json",
      ]
    }
  }
}
```

#### Windows Configuration (via WSL)

**Config file location**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "fs-text-search": {
      "command": "wsl.exe",
      "args": [
        "bash",
        "-c",
        "/usr/bin/docker run -i --rm -v /mnt/c/path/to/your/documents:/home/source -v /mnt/c/path/to/search/index:/home/index foobarxyzzy/fs-text-search-mcp --extensions txt,md,rs,py,js,ts,json"
      ]
    }
  }
}
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|--------|
| `--extensions` | `-e` | File extensions to include (comma-separated) | `txt,md` |
| `--verbose` | `-v` | Enable verbose logging | false |
| `--help` | `-h` | Show help message | - |

### Contribute

#### Run in local

```bash
$ cargo run
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|--------|
| `--watch-dir` | `-w` | Directory to watch for file changes | `./` |
| `--index-dir` | `-i` | Directory to store search index (optional) | In-memory |
| `--extensions` | `-e` | File extensions to include (comma-separated) | `txt,md` |
| `--verbose` | `-v` | Enable verbose logging | false |

#### Example MCP Interactions

```json
// Initialize the connection
{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}

// Get available tools
{"jsonrpc":"2.0","method":"tools/list","id":1}

// Search for content
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_index","arguments":{"keyword":"function"}}}

// Load file
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"load_file","arguments":{"file_path":"/path/to/file"}}}
```

### Dependencies

- **tantivy**: Full-text search engine
- **notify-debouncer-full**: File system event debouncing
- **rmcp**: Model Context Protocol implementation
- **clap**: Command-line argument parsing
- **tokio**: Async runtime
- **tracing**: Structured logging