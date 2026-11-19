<p align="center">
  <img src="https://raw.githubusercontent.com/itskum47/obscura/main/assets/icon.png" alt="Obscura Logo" width="80" />
</p>

<h1 align="center">Obscura</h1>

<p align="center">
  <strong>An open-source browser engine for AI agents, web automation, and scraping.</strong><br>
  Built in Rust &bull; Embedded V8 &bull; Native Rendering &bull; Zero Chromium Dependency
</p>

<p align="center">
  <a href="https://github.com/itskum47/obscura/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-Apache%202.0-2b2b2b?style=for-the-badge" alt="License" /></a>
  <a href="https://github.com/itskum47/obscura"><img src="https://img.shields.io/badge/Rust-2b2b2b?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" /></a>
  <a href="https://github.com/itskum47/obscura"><img src="https://img.shields.io/badge/V8-Embedded-2b2b2b?style=for-the-badge" alt="V8" /></a>
</p>

---

## What Is Obscura?

Obscura is an independent, lightweight browser engine written from scratch in Rust. It embeds Google V8 directly alongside an independent native layout and rasterization engine — no Chromium, no WebDriver, no desktop browser overhead.

It provides a unified set of interfaces for AI agents, programmatic scraping, and browser automation:

- **CLI** — `obscura fetch`, `scrape`, `render`, `serve`, `doctor`
- **MCP Server** — 41 browser tools over stdio and HTTP/SSE for AI agents
- **CDP Server** — Chrome DevTools Protocol over WebSocket for Puppeteer/Playwright
- **Rust Library** — ergonomic `Browser`, `Page`, `CookieStore` API

Obscura is designed for content extraction, agent interaction, and layout verification. It is **not** an Electron wrapper or re-packaged Chromium binary.

---

## Architecture

```mermaid
flowchart TD
    %% ==========================================
    %% 1. CLIENTS & INTEGRATION LAYER
    %% ==========================================
    subgraph TIER1["1. Clients & Agent Ecosystem"]
        Agents["AI Agents & LLM Runtimes\n(Claude, GPT-4, Gemini, LangChain, AutoGen, CrewAI)"]
        MCP_Hosts["MCP Host Applications\n(Cursor, Claude Desktop, Windsurf, Agent Harnesses)"]
        SDK_Py["Python SDK: obscura-next\n(sdk/python: async ObscuraClient)"]
        SDK_Ts["TypeScript SDK: @obscura-next/client\n(sdk/typescript: child_process IPC)"]
        CLI_Bin["Obscura CLI\n(crates/obscura-cli: fetch, scrape, render, serve, doctor)"]
    end

    %% ==========================================
    %% 2. INGRESS & PROTOCOL GATEWAYS
    %% ==========================================
    subgraph TIER2["2. Ingress & Protocols"]
        MCP_Srv["MCP Server (crates/obscura-mcp)\n- Transports: Stdio & HTTP/SSE\n- High-Level: research_web, web_search, fetch_url, compare_sources\n- Low-Level: 41 Browser Automation Tools (Click, Type, Eval, Snapshot)"]
        CDP_Srv["CDP Protocol Server (crates/obscura-cdp)\n- 14 Domains: Page, DOM, Network, Target, Runtime, Fetch\n- WebSocket & HTTP Endpoints for Playwright/Puppeteer"]
        Worker_IPC["Worker IPC (crates/obscura-worker)\n- Framed JSON Lines over Stdin/Stdout\n- Sandboxed Process Actor Session Isolation"]
        Facade_Lib["Root Facade (crates/obscura)\n- Ergonomic Rust Library API: Browser, Page, CookieStore"]
    end

    %% ==========================================
    %% 3. INTELLIGENCE & ORCHESTRATION
    %% ==========================================
    subgraph TIER3["3. Autonomous Web Intelligence & Decision Engine (crates/obscura-agent)"]
        Orchestrator["Research Orchestrator\n- Objective Decomposition & Subtask Planning\n- DAG Scheduling & State Tracking\n- Provenance & Citation Linking"]
        Discovery["Discovery Engine\n- Search Providers: DuckDuckGo, Brave, SearXNG, Feeds\n- URL Normalizer: Strips utm_*, fbclid, tracking tokens\n- Allowlist & Denylist Domain Filtering"]
        Relevance["Explainable Relevance Pipeline\n- BM25 Term Salience over Title & Main Body\n- Completeness & Freshness Scoring\n- Transparent Scoring Breakdown (No Black Box)"]
        Aggregator["Multi-Source Aggregator\n- Cross-Source Fact Clustering (Jaccard Similarity)\n- Contradiction & Numeric Discrepancy Detection\n- Sourced Claims with Direct URL Citations"]
        Router["Intelligent Execution Router\n- Analyzes Response Headers, Status & Initial HTML\n- Probes: SPA Containers (#root, #app), noscript gates\n- Directs traffic to Fast Path vs Browser Escalation"]
    end

    %% ==========================================
    %% 4. DUAL-ENGINE EXECUTION PIPELINE
    %% ==========================================
    subgraph TIER4["4. Dual-Engine Execution Pipeline"]
        subgraph PATH_A["Path A: Pure-Rust Lightweight Intelligence (< 20ms, Zero V8, ~0 MB Heap)"]
            Net["HTTP Transport (crates/obscura-net)\n- reqwest + rustls (TLS 1.3)\n- robots.txt Evaluator & Cache\n- PGL Ad & Tracker Domain Blocklist\n- Multi-Charset Sniffer: GBK, Big5, EUC-JP, UTF-8\n- Custom CookieJar Scoping"]
            SSRF["SSRF Defense Guard (crates/obscura-ssrf)\n- Denies Loopback: 127.0.0.0/8, ::1\n- Denies RFC 1918 Private: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16\n- Denies Cloud Metadata: 169.254.169.254\n- Pre-Connect DNS Resolution Rebinding Check"]
            DOM_Engine["Pure-Rust DOM (crates/obscura-dom)\n- Arena-Allocated DomTree (html5ever)\n- CSS Selectors: cssparser & selectors\n- Shadow DOM Trees (Open/Closed & Slotting)\n- Zero V8 / Zero C++ Dependencies"]
            Extractor["Extraction Engine (crates/obscura-extract)\n- Arc90 Readability Scoring & Boilerplate Stripper\n- OpenGraph, Twitter Cards & Canonical Meta\n- Schema.org JSON-LD Normalization (Articles, Products, FAQs)\n- HTML Tables to Structured Markdown & JSON\n- XML Sitemap & RSS/Atom Feed Parsers (roxmltree)"]
        end

        subgraph PATH_B["Path B: Supervised Headless Browser Engine (V8 Escalation, ~4.6 MB RSS)"]
            Browser_Core["Browser Engine (crates/obscura-browser)\n- Page Lifecycle & Multi-Tab Contexts\n- Navigation State Machine & History Traversal\n- Network Quiescence & Settling Detection\n- CSS Font-Face Pre-Warming"]
            V8_Runtime["Embedded V8 Runtime (crates/obscura-js)\n- deno_core v0.412.0 Isolate Hosting\n- Web APIs: Fetch, XHR, Storage, WebCrypto, Timers\n- Watchdog Timeout & Anti-Panic Unwind Catching"]
            Render_Core["2D Layout & Rasterizer (crates/obscura-render)\n- Taffy v0.12 Pure-Rust Flexbox & Grid\n- Font Shaping: cosmic-text & ab_glyph\n- tiny-skia CPU Software 2D Paint\n- Pixel Screenshots & DOM Bounding Boxes"]
            OS_Isolation["OS Sandboxing (crates/obscura-isolation)\n- Linux Landlock Filesystem Access Rules\n- Linux Seccomp System Call Filtering\n- macOS Seatbelt Security Profiles\n- Strict rlimit Memory & File-Descriptor Caps"]
        end
    end

    %% ==========================================
    %% 5. DISTRIBUTED CONTROL PLANE & INFRASTRUCTURE
    %% ==========================================
    subgraph TIER5["5. Distributed Control Plane & Worker Infrastructure"]
        Scheduler["Distributed Scheduler (crates/obscura-scheduler)\n- Worker Capacity Matching & Balancing\n- Heartbeat Monitoring & Lease Reaper\n- Fencing Token Validation"]
        Worker_Agent["Worker Daemon (crates/obscura-worker-agent)\n- Subscribes to NATS Dispatch Queues\n- Lease Acquisition & Fencing Enforcement\n- Supervises Local Execution Nodes"]
        Supervisor["Process Supervisor (crates/obscura-supervisor)\n- PR_SET_PDEATHSIG Child Process Binding\n- Process Group (pgid) SIGKILL Containment\n- RSS Memory Accounting & Auto-Recycling"]
        Telemetry["Telemetry Exporter (crates/obscura-telemetry)\n- Prometheus Metrics Registry (Bearer Auth)\n- OpenTelemetry Distributed Tracing\n- Liveness /healthz & /readyz Probes"]
        Postgres[("PostgreSQL 16 Cluster\n- Row-Level Locking (SKIP LOCKED)\n- Transactional Outbox Pattern\n- Multi-Tenant Isolation & Audit Trail")]
        NATS[("NATS JetStream 2.10\n- Durable Job Queues (jobs.dispatched)\n- Worker Heartbeats & Completion Events")]
    end

    %% ==========================================
    %% CONNECTIONS & FLOWS
    %% ==========================================
    %% Ingress connections
    Agents --> MCP_Srv
    MCP_Hosts --> MCP_Srv
    SDK_Py --> Worker_IPC
    SDK_Ts --> Worker_IPC
    CLI_Bin --> Ingress_Route{"Command Route"}
    Ingress_Route -->|fetch/scrape| Net
    Ingress_Route -->|render| Render_Core
    Ingress_Route -->|mcp| MCP_Srv
    Ingress_Route -->|serve| CDP_Srv
    Ingress_Route -->|worker| Worker_IPC

    %% Ingress to Logic
    MCP_Srv --> Orchestrator
    MCP_Srv --> Browser_Core
    CDP_Srv --> Browser_Core
    Worker_IPC --> Browser_Core
    Facade_Lib --> Browser_Core
    Facade_Lib --> Net

    %% Intelligence Flow
    Orchestrator --> Discovery
    Discovery --> Router

    %% Routing Decisions
    Router -->|1. Static Content / Informational Article| Net
    Router -->|2. Client SPA / Dynamic JS Required| Browser_Core
    Router -->|3. Structured Feeds / Sitemap / API| Extractor

    %% Path A Wiring
    Net --> SSRF
    Net --> DOM_Engine
    DOM_Engine --> Extractor
    Extractor --> Relevance
    Relevance --> Aggregator
    Aggregator --> Orchestrator

    %% Path B Wiring
    Browser_Core --> DOM_Engine
    Browser_Core --> V8_Runtime
    Browser_Core --> Render_Core
    Browser_Core --> OS_Isolation
    Browser_Core --> Net
    V8_Runtime -.-> DOM_Engine

    %% Distributed Control Plane Wiring
    MCP_Srv -.->|Async Job Submission| Postgres
    CLI_Bin -.->|Async Batch Submission| Postgres
    Scheduler -->|Read Outbox: FOR UPDATE SKIP LOCKED| Postgres
    Scheduler -->|Publish job.dispatched| NATS
    Worker_Agent -->|Pull Work Assignment| NATS
    Worker_Agent -->|Acquire & Renew Lease: fencing_token + 1| Postgres
    Worker_Agent --> Supervisor
    Supervisor --> Worker_IPC
    Supervisor --> Telemetry
    Scheduler --> Telemetry
```

---

## Key Capabilities

- **Embedded JavaScript Engine:** Executes real-world JavaScript through Google V8 (`deno_core`) with standard ECMAScript semantics, microtask scheduling, and DOM event dispatching.
- **Native Layout & Paint Engine:** Renders pages, captures screenshots, and exports PDFs without Chromium using a dedicated Rust layout tree (Block, Inline, Flexbox, Grid) and 2D rasterizer.
- **AI Agent Automation (MCP):** Implements the Model Context Protocol (MCP) natively over `stdio` and `HTTP/SSE`, providing 41 purpose-built browser automation and extraction tools.
- **CDP Server Compatibility:** Exposes Chrome DevTools Protocol WebSocket endpoints compatible with standard Puppeteer and Playwright automation scripts.
- **Content Extraction:** Pure-Rust readability engine strips navigation, sidebars, and boilerplate. Extracts clean Markdown, metadata (OpenGraph, Twitter Cards, canonical URLs, author, published time), JSON-LD structured data (articles, products, FAQs), tables, and hyperlinks — no V8 overhead.
- **High-Throughput Parallel Scraping:** Built-in multi-process worker pool (`obscura scrape`) designed to extract content across thousands of URLs concurrently with managed memory recycling.
- **Enterprise Isolation & Sandboxing:** Default SSRF protection against private networks, combined with Linux container isolation (namespaces, Landlock LSM, Seccomp-BPF) and macOS Seatbelt confinement.
- **Distributed Control Plane:** PostgreSQL-backed job queue with row-level locking, transactional outbox, NATS JetStream dispatch, lease fencing, and multi-process worker supervision.

---

## Quick Start

### 1. Verify Installation and Engine Health
```bash
obscura doctor
```

### 2. Fetch and Extract Content
```bash
# Convert readable page content to clean Markdown
obscura fetch https://example.com --dump markdown

# Extract all hyperlinks from the document
obscura fetch https://example.com --dump links

# Output rendered DOM HTML
obscura fetch https://news.ycombinator.com --dump html

# Evaluate JavaScript in-page
obscura fetch https://example.com --eval "document.title"
```

### 3. Capture a Screenshot
```bash
obscura fetch https://example.com --screenshot page.png
```

### 4. Run as MCP Server (for AI Agents)
```bash
# stdio transport (Claude Desktop, Cursor, Windsurf)
obscura mcp

# HTTP/SSE transport
obscura mcp --transport http --port 8080
```

### 5. Run as CDP Server (for Puppeteer/Playwright)
```bash
obscura serve --port 9222
```

---

## Demo: Autonomous Web Research via MCP

Connect Obscura to Claude Desktop or any MCP client and ask it to research a topic:

```
You: Research the latest developments in Rust async runtimes

Claude (via Obscura MCP):
  → research_web("rust async runtimes")
  → Searches DuckDuckGo for relevant sources
  → Fetches and extracts content from each source
  → Ranks by BM25 relevance
  → Returns a synthesized report with citations

Result: A cited research report with sources, claims, and contradictions —
all from a self-hosted 4 MB binary. No Chromium. No cloud API. No tokens.

**Example output (abbreviated):**
```json
{
  "report": {
    "objective": "rust programming language",
    "executive_summary": "Synthesis across 3 consulted sources... Key consensus indicates: Rust is blazingly fast and memory-efficient...",
    "verified_claims": [
      { "statement": "Rust enforces memory safety without a garbage collector", "consensus_count": 2, "citations": [...] }
    ],
    "sources": [
      { "url": "https://www.rust-lang.org/", "title": "Official site", "relevance_score": 0.92, "retrieval_method": "http" },
      { "url": "https://en.wikipedia.org/wiki/Rust_(programming_language)", "title": "Rust (programming language)", "relevance_score": 0.87, "retrieval_method": "http" }
    ],
    "contradictions": [],
    "limitations": []
  },
  "http_retrieval_count": 3,
  "elapsed_ms": 4200
}
```

**Try it yourself:**
```bash
# 1. Install
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh

# 2. Add to Claude Desktop config
# macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
# {
#   "mcpServers": {
#     "obscura": { "command": "obscura", "args": ["mcp"] }
#   }
# }

# 3. Ask Claude: "Research the latest developments in Rust async runtimes"
```

---

## Installation

### One-Line Install (macOS / Linux)
```bash
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/itskum47/obscura/main/install.ps1 | iex
```

### From Source
```bash
git clone https://github.com/itskum47/obscura.git
cd obscura
cargo build --release
```

---

## Performance

| Metric | Obscura | Headless Chromium |
|--------|---------|-------------------|
| Startup latency | ~8.5 ms | ~500 ms+ |
| Memory (idle) | ~4.3 MB RSS | ~150–300 MB |
| Memory (active session) | ~4.6 MB RSS | ~200–400 MB |
| 8 concurrent workers | ~37 MB total | ~1.2–2.4 GB |
| HTML parse (50 KB) | ~24 ms | ~50–100 ms |

*Obscura runs a full V8 isolate + Rust DOM in ~4 MB. No Chromium, no WebDriver, no desktop browser overhead.*

---

## SDKs

### Python
```python
from obscura_next import ObscuraClient

async with ObscuraClient() as client:
    await client.navigate("https://example.com")
    title = await client.eval("document.title")
    print(title)
```

### TypeScript / Node.js
```typescript
import { ObscuraClient } from "@obscura-next/client";

const client = new ObscuraClient();
await client.start();
await client.navigate("https://example.com");
const title = await client.eval("document.title");
console.log(title);
await client.stop();
```

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Installation](docs/Installation.md) | Detailed installation across all platforms |
| [CLI Reference](docs/CLI-reference.md) | Full command and flag reference |
| [MCP Server](docs/Use-the-MCP-server.md) | MCP setup for Claude, Cursor, Windsurf |
| [CDP Integration](docs/Connect-Puppeteer-or-Playwright.md) | Puppeteer/Playwright connection guide |
| [Extract Data](docs/Extract-data.md) | Content extraction and structured data |
| [Markdown Extraction](docs/Markdown-extraction.md) | Markdown output formats |
| [Build from Source](docs/Build-from-source.md) | Compiling from source |
| [Architecture](docs/Architecture-overview.md) | System architecture overview |
| [Environment Variables](docs/Environment-variables.md) | Configuration reference |

---

## Project Structure

```
obscura-next/
├── crates/
│   ├── obscura-dom/          # Pure-Rust DOM (html5ever, CSS selectors)
│   ├── obscura-net/          # HTTP transport, SSRF, cookies, robots.txt
│   ├── obscura-ssrf/         # SSRF deny-set & private network validation
│   ├── obscura-browser/      # Page lifecycle, V8 isolate integration
│   ├── obscura-js/           # Embedded V8 runtime (deno_core)
│   ├── obscura-cdp/          # Chrome DevTools Protocol server
│   ├── obscura-mcp/          # Model Context Protocol server
│   ├── obscura-render/       # Taffy layout + tiny-skia rasterizer
│   ├── obscura-extract/      # Readability, JSON-LD, metadata, tables, links, feeds
│   ├── obscura-agent/        # Research orchestrator, relevance, routing, aggregation
│   ├── obscura-cli/          # CLI binary (fetch, scrape, serve, mcp, doctor)
│   ├── obscura/              # Root facade library
│   ├── obscura-telemetry/    # Prometheus metrics, OpenTelemetry tracing
│   ├── obscura-isolation/    # OS sandboxing (Landlock, Seccomp, Seatbelt)
│   ├── obscura-worker/       # Worker process & session isolation
│   ├── obscura-supervisor/   # Process supervision tree
│   ├── obscura-control-plane/# PostgreSQL control plane
│   ├── obscura-scheduler/    # Distributed job scheduler
│   └── obscura-worker-agent/ # NATS worker daemon
├── sdk/
│   ├── python/               # Python SDK (async/sync)
│   └── typescript/           # TypeScript/Node.js SDK
├── docs/                     # Documentation
├── deploy/                   # Docker & deployment configs
└── tests/                    # Integration test harnesses
```

---

## Contributing

Contributions are welcome. Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Obscura is licensed under the [Apache License 2.0](LICENSE).

---

<p align="center">
  Built with 🦀 Rust &bull; Embedded V8 &bull; Zero Chromium
</p>