# MCP Server for Full Text Search in local file

## Usage

## Development

### Check in local

```sh
$ cargo run

# Initialize request
$ {"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0.0"},"capabilities":{}}}
# Notify initialized
$ {"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
# Get tools
$ {"jsonrpc":"2.0","method":"tools/list","id":1}
# Search
$ {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_index","arguments":{"keyword":"function"}}}
```