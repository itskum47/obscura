# Obscura MCP Onboarding Guide

Connect your AI assistants and agents to the web using the official Obscura Model Context Protocol (MCP) server.

Obscura provides a native, standalone MCP server that requires **no external Chrome or Chromium installation**. It runs fully contained in Rust and provides 41 web automation and extraction tools.

---

## 1. Quick Verification

Before configuring your MCP client, ensure `obscura` is installed and verified:

```bash
obscura doctor
```

Verify that the check `[PASS] Model Context Protocol (MCP)` reports operational status.

Test running the server via stdio:
```bash
obscura mcp --help
```

---

## 2. Client Configurations

### A. Claude Desktop

Add Obscura to your Claude Desktop configuration file:

- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Linux:** `~/.config/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

#### Recommended Configuration (Standard Stdio)

```json
{
  "mcpServers": {
    "obscura": {
      "command": "obscura",
      "args": ["mcp"]
    }
  }
}
```

#### Hardened & Stealth Configuration

To enable anti-bot stealth emulation and persistent cookies:

```json
{
  "mcpServers": {
    "obscura": {
      "command": "obscura",
      "args": [
        "mcp",
        "--stealth",
        "--storage-dir",
        "/Users/username/.local/share/obscura/profile"
      ]
    }
  }
}
```

*(Replace `/Users/username` with your home directory path)*

---

### B. Cursor

In Cursor:
1. Open **Cursor Settings** (`Ctrl+,` or `Cmd+,`).
2. Navigate to **Features** -> **MCP Servers**.
3. Click **Add New MCP Server**.
4. Configure:
   - **Name:** `obscura`
   - **Type:** `command`
   - **Command:** `obscura mcp`

Alternatively, add to `.cursor/mcp.json` in your workspace:

```json
{
  "mcpServers": {
    "obscura": {
      "command": "obscura",
      "args": ["mcp"]
    }
  }
}
```

---

### C. Zed Editor

Add Obscura to `~/.config/zed/settings.json` (or `settings.json` via `Cmd+,`):

```json
{
  "context_servers": {
    "obscura": {
      "command": {
        "path": "obscura",
        "args": ["mcp"]
      }
    }
  }
}
```

---

### D. Windsurf & Custom Agent Frameworks

Obscura supports standard stdio MCP and Streamed HTTP SSE transport.

#### HTTP / Remote MCP Server
If your agent runs inside Docker or a remote container, launch Obscura in HTTP server mode:

```bash
obscura mcp --http --host 127.0.0.1 --port 3000
```

Configure your client to point to `http://127.0.0.1:3000/sse`.

---

## 3. Available Agent Tools

Obscura exposes 41 tools to connected LLMs, grouped into core categories:

| Tool | Description |
| :--- | :--- |
| `browser_navigate` | Navigate active tab to any HTTP(S) URL |
| `browser_snapshot` | Capture semantic accessibility DOM snapshot with ref IDs (`e1`, `e2`) |
| `browser_click` | Click interactive elements using stable snapshot ref ID |
| `browser_fill` | Fill text forms or inputs by element ref |
| `browser_type` | Type text character-by-character into active element |
| `browser_press_key` | Simulate keypresses (Enter, Tab, Escape, Arrow keys) |
| `browser_evaluate` | Run arbitrary JavaScript expressions within the page isolate |
| `browser_screenshot` | Capture page layout as PNG (render-enabled builds) |
| `browser_tab_list` / `browser_tab_new` | Inspect and manage concurrent browser tabs |
| `browser_get_cookies` / `browser_set_cookie` | Read, inspect, and set cookie headers across sessions |

### Ref-Based Navigation Pattern
Unlike brittle CSS selector scripts, Obscura provides ref-based DOM snapshots:
1. Agent calls `browser_snapshot`.
2. Obscura returns accessible text with short handles (`[e1] Submit`, `[e2] Search Input`).
3. Agent invokes `browser_click({ "ref": "e1" })`.
4. Obscura resolves the element directly in the Rust DOM tree.

---

## 4. Verification & Diagnostics

To test interactive MCP communication manually:

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}\n' | obscura mcp
```

Expected output: A JSON-RPC response listing available tool definitions, schemas, and descriptions.
