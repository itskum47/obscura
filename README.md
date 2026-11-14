<p align="center">
  <img src="https://raw.githubusercontent.com/itskum47/obscura/main/assets/icon.png" alt="Obscura Logo" width="80" />
</p>

<h1 align="center">Obscura</h1>

<p align="center">
  <strong>An open-source browser engine for AI agents, web automation, and scraping.</strong><br>
  Built in Rust &bull; Embedded V8 &bull; Native Rendering &bull; Zero Chromium Dependency
</p>

<p align="center">
  <a href="docs/README.md"><img src="https://img.shields.io/badge/Documentation-2b2b2b?style=for-the-badge&logo=gitbook&logoColor=white" alt="Documentation" /></a>
  <a href="https://obscura.sh"><img src="https://img.shields.io/badge/Website-2b2b2b?style=for-the-badge&logoColor=white" alt="Website" /></a>
  <a href="https://github.com/itskum47/obscura/releases"><img src="https://img.shields.io/badge/Releases-2b2b2b?style=for-the-badge&logo=github&logoColor=white" alt="Releases" /></a>
  <a href="docs/mcp/Onboarding.md"><img src="https://img.shields.io/badge/MCP_Onboarding-2b2b2b?style=for-the-badge&logo=anthropic&logoColor=white" alt="MCP Onboarding" /></a>
  <a href="https://github.com/itskum47/obscura-benchmark"><img src="https://img.shields.io/badge/Benchmarks-2b2b2b?style=for-the-badge&logo=speedtest&logoColor=white" alt="Benchmarks" /></a>
  <a href="https://tally.so/r/gDWzdD"><img src="https://img.shields.io/badge/Cloud_Waitlist-2b2b2b?style=for-the-badge&logo=googlecloud&logoColor=white" alt="Cloud Waitlist" /></a>
</p>

---

## What Is Obscura?

Obscura is an independent, lightweight browser engine written from scratch in Rust, designed specifically for AI agents, programmatic web scraping, and browser automation. 

Unlike conventional headless automation tools that run and control an entire desktop Google Chrome or Chromium installation, Obscura embeds Google V8 directly alongside an independent native layout and rasterization engine. It provides unified automation interfaces without the operational overhead, memory consumption, or background telemetry of desktop browser suites.

```
┌────────────────────────────────────────────────────────────────────────┐
│                          Unified Interfaces                            │
│     CLI (`obscura`)   │   MCP Server (AI Agents)   │   CDP Server      │
├────────────────────────────────────────────────────────────────────────┤
│                           Obscura Engine                               │
│  ┌──────────────────────┐  ┌─────────────────┐  ┌───────────────────┐  │
│  │ Embedded V8 Runtime  │  │  Rust DOM & CSS │  │  Native Layout &  │  │
│  │ (JS, Promises, DOM)  │  │  (html5ever)    │  │  Paint Engine     │  │
│  └──────────────────────┘  └─────────────────┘  └───────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ Security & Isolation Boundaries: SSRF Gate, Namespaces, Landlock │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

### Key Capabilities
- **Embedded JavaScript Engine:** Executes real-world JavaScript through Google V8 (`deno_core`) with standard ECMAScript semantics, microtask scheduling, and DOM event dispatching.
- **Native Layout & Paint Engine:** Renders pages, captures screenshots, and exports PDFs without Chromium using a dedicated Rust layout tree (Block, Inline, Flexbox, Grid) and 2D rasterizer.
- **AI Agent Automation (MCP):** Implements the Model Context Protocol (MCP) natively over `stdio` and `HTTP` transports, providing 41 purpose-built automation and extraction tools.
- **CDP Server Compatibility:** Exposes Chrome DevTools Protocol WebSocket endpoints compatible with standard Puppeteer and Playwright automation scripts.
- **High-Throughput Parallel Scraping:** Built-in multi-process worker pool (`obscura scrape`) designed to extract content across thousands of URLs concurrently with managed memory recycling.
- **Enterprise Isolation & Sandboxing:** Default SSRF protection against private networks, combined with Linux container isolation (namespaces, Landlock LSM, Seccomp-BPF) and macOS Seatbelt confinement.

> [!NOTE]
> Obscura is an automated headless browser engine built for programmatic data extraction and AI agency. It is **not** an Electron wrapper or re-packaged Chromium binary, and it does not seek 100% bug-for-bug compatibility with desktop Chrome interactive UI features.

---

## Why Obscura?

| Engineering Characteristic | Obscura | Headless Chromium / Chrome |
| :--- | :--- | :--- |
| **Architecture** | Memory-safe native Rust browser engine | Monolithic C++ desktop browser codebase |
| **Startup Overhead** | Instantaneous cold start (milliseconds) | Process fork, GPU process, utility processes (~1–2s) |
| **Memory Footprint** | Tens of megabytes per active session | 150–300+ MB per tab instance |
| **AI Agent Support** | Native built-in MCP server (stdio & HTTP) | Requires external bridge or Node.js wrapper |
| **External Dependencies** | Zero (single standalone binary) | Heavy system packages, fonts, shared libraries |
| **Default Network Security** | Built-in SSRF gate (private subnets blocked) | Unrestricted network access unless externally sandboxed |
| **Anti-Fingerprinting** | Built-in TLS and browser fingerprint randomization | Requires external stealth plugins (e.g. puppeteer-extra) |

*For exact, reproducible benchmark methodology and test conditions across our 33-stage obstacle course, visit the [obscura-benchmark repository](https://github.com/itskum47/obscura-benchmark).*

---

## Installation

Obscura is distributed as a single standalone executable paired with its worker binary (`obscura` and `obscura-worker`). No Node.js, Chromium, or system browser installation is required.

### 1. macOS & Linux Native Installer

Run the official POSIX installer in your terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh
```

The installer detects your operating system and architecture, validates the release archive against official SHA-256 checksums, installs both binaries to `~/.local/bin`, and verifies executable permissions.

#### Installer Options
Pass flags directly using `sh -s -- [options]`:

```bash
# Install a specific pinned version
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --version v0.3.0

# Install the stealth variant with TLS impersonation
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --variant stealth

# Custom installation directory
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --install-dir /opt/obscura/bin
```

> [!IMPORTANT]
> **macOS Security & Gatekeeper Notice:**
> Official prebuilt macOS binaries are ad-hoc signed and unnotarized. The installer preserves Apple Gatekeeper quarantine flags by default. If macOS Gatekeeper prevents execution on first launch, authorize Obscura in **System Settings $\to$ Privacy & Security**, or manually clear the attribute:
> ```bash
> xattr -d com.apple.quarantine ~/.local/bin/obscura ~/.local/bin/obscura-worker
> ```

---

### 2. Windows PowerShell Installer

Open Windows PowerShell (administrator privileges are not required):

```powershell
irm https://raw.githubusercontent.com/itskum47/obscura/main/install.ps1 | iex
```

The installer verifies SHA-256 signatures, extracts binaries to `%LOCALAPPDATA%\Obscura\bin`, integrates with your User `PATH` (with case-insensitive deduplication), and supports atomic transactional rollback if installation is interrupted.

#### PowerShell Parameters
```powershell
.\install.ps1 -Version "v0.3.0" -Variant "stealth" -InstallDir "C:\Tools\Obscura"
```

---

### 3. Docker Container

Obscura publishes hardened, minimal container images based on Google's `distroless/cc-debian12:nonroot` with zero shell or package manager in the runtime:

```bash
# Generate a secure authentication token
TOKEN=$(openssl rand -hex 32)

# Run container bound to host loopback
docker run -d \
  --name obscura \
  -p 127.0.0.1:9222:9222 \
  -e OBSCURA_CDP_TOKEN="$TOKEN" \
  ghcr.io/itskum47/obscura:latest
```

> [!CAUTION]
> Because containerized services bind `0.0.0.0` internally, `obscura serve` strictly requires setting `OBSCURA_CDP_TOKEN`. Never expose unauthenticated CDP ports to public networks.

---

### 4. Manual Release Downloads

Download prebuilt archives and cryptographic checksums directly from [GitHub Releases](https://github.com/itskum47/obscura/releases):

| Platform | Architecture | Archive Name | Checksum Tool |
| :--- | :--- | :--- | :--- |
| **Linux** | x86_64 | `obscura-x86_64-linux.tar.gz` | `sha256sum --check --ignore-missing checksums.txt` |
| **Linux** | ARM64 (`aarch64`) | `obscura-aarch64-linux.tar.gz` | `sha256sum --check --ignore-missing checksums.txt` |
| **macOS** | Apple Silicon (`aarch64`) | `obscura-aarch64-macos.tar.gz` | `shasum -a 256 -c checksums.txt --ignore-missing` |
| **macOS** | Intel (`x86_64`) | `obscura-x86_64-macos.tar.gz` | `shasum -a 256 -c checksums.txt --ignore-missing` |
| **Windows** | x86_64 | `obscura-x86_64-windows.zip` | `Get-FileHash .\obscura-x86_64-windows.zip -Algorithm SHA256` |

Available variant suffixes: `default` (standard), `-stealth` (TLS fingerprinting + tracker blocking), `-no-render` (fast headless DOM traversal), and `-no-render-stealth`.

---

### 5. Build from Source

To compile Obscura natively with Cargo:

```bash
# Clone the repository
git clone https://github.com/itskum47/obscura.git
cd obscura

# Build with rendering engine enabled (recommended)
cargo build --release -p obscura-cli --bins --features render

# Build with rendering and stealth TLS impersonation
cargo build --release -p obscura-cli --bins --features render,stealth
```

Compiled binaries will be located at `target/release/obscura` and `target/release/obscura-worker`. See [docs/Build-from-source.md](docs/Build-from-source.md) for build dependencies.

---

## Quick Start

### 1. Verify Installation and Engine Health
Run `obscura doctor` to verify V8 runtime initialization, rendering engine rasterization, MCP tool loading, loopback socket bindings, and host platform containment:

```bash
obscura doctor
```

```text
================================================================================
                           Obscura Diagnostic Doctor
================================================================================
Version:         0.3.0
Platform:        macos (aarch64)
Features:        render=enabled, stealth=disabled

System & Subsystem Diagnostics:
  [PASS] CLI Binary & Version
  [PASS] Worker Process Binary
  [PASS] JavaScript Engine (V8)
  [PASS] Rendering & Layout Engine
  [PASS] Model Context Protocol (MCP) (41 tools verified)
  [PASS] Loopback Network Binding
  [PASS] Storage & Cache Permissions
  [WARN] Host Kernel Sandboxing Capabilities (Darwin Seatbelt active; Linux required for multi-tenant untrusted)
  [PASS] Process Confinement & Profile Enforcement (Diagnostic CLI unconfined; profile-driven on workers)

Summary: 8 passed, 1 notice(s), 0 failed.
================================================================================
```

### 2. Fetch and Evaluate JavaScript
```bash
# Retrieve page title via in-process V8 evaluation
obscura fetch https://example.com --eval "document.title"
```

### 3. Extract Structured Content
```bash
# Convert readable page content directly to clean Markdown
obscura fetch https://example.com --dump markdown

# Extract all absolute hyperlinks from the document
obscura fetch https://example.com --dump links

# Output rendered DOM HTML
obscura fetch https://news.ycombinator.com --dump html
```

### 4. Capture a Screenshot
```bash
# Render layout and save full viewport as PNG (requires render feature)
obscura fetch https://example.com --screenshot page.png
```

### 5. Inspect Subcommand Options
```bash
obscura --help
obscura fetch --help
obscura serve --help
obscura mcp --help
```

---

## Choose Your Integration

Obscura provides four primary integration paths depending on your workflow:

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Integration Options                             │
├───────────────────┬───────────────────┬────────────────────────────────┤
│ 1. CLI Browser    │ 2. MCP for Agents │ 3. CDP (Puppeteer/Playwright)  │
│ Terminal scripts, │ Claude Desktop,   │ Existing Node.js & Python CDP  │
│ batch scraping,   │ Cursor, Zed,      │ automation scripts and testing │
│ data dumps        │ LLM frameworks    │ suites                         │
├───────────────────┴───────────────────┴────────────────────────────────┤
│ 4. Native SDKs (Python & TypeScript)                                   │
│ Direct programmatic browser orchestration                              │
└────────────────────────────────────────────────────────────────────────┘
```

---

### 1. CLI (Command-Line Interface)

Use the CLI for shell scripts, one-off scraping tasks, and high-concurrency batch processing.

```bash
# Fetch and wait for network activity to settle
obscura fetch https://example.com --wait-until networkidle0 --timeout 15

# Route requests through an upstream HTTP or SOCKS5 proxy
obscura --proxy socks5://127.0.0.1:1080 fetch https://example.com --dump text

# Scrape multiple URLs concurrently with parallel worker processes
obscura scrape https://news.ycombinator.com https://en.wikipedia.org \
  --concurrency 10 \
  --eval "document.title" \
  --format json
```

*See [docs/CLI-reference.md](docs/CLI-reference.md) for full argument details.*

---

### 2. MCP for AI Agents (Model Context Protocol)

The Model Context Protocol (MCP) connects LLM assistants directly to external tools. Obscura implements a native MCP server exposing **41 browser automation tools** (navigation, accessibility snapshots, clicking, typing, form submission, evaluation, tab management, cookie control, and network request inspection).

#### Transports
- **stdio (Default):** Used when the MCP client spawns Obscura directly as a subprocess.
  ```bash
  obscura mcp
  ```
- **HTTP Transport:** Used when clients connect over standard HTTP POST.
  ```bash
  obscura mcp --http --port 8080
  # JSON-RPC endpoint: http://127.0.0.1:8080/mcp
  ```

#### Claude Desktop Configuration
Add Obscura to your `claude_desktop_config.json`:

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

*For setup guides covering Cursor, Zed, Windsurf, and complete tool references, visit [docs/mcp/Onboarding.md](docs/mcp/Onboarding.md) and [docs/mcp/TOOLS.md](docs/mcp/TOOLS.md).*

---

### 3. Chrome DevTools Protocol (CDP)

Obscura runs a standards-compliant WebSocket CDP server, allowing you to connect existing Puppeteer and Playwright scripts without installing Chrome.

#### Start the CDP Server
```bash
obscura serve --port 9222
```

#### Puppeteer Connection
```javascript
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.connect({
  browserWSEndpoint: 'ws://127.0.0.1:9222/devtools/browser',
});

const page = await browser.newPage();
await page.goto('https://news.ycombinator.com');

const stories = await page.evaluate(() =>
  Array.from(document.querySelectorAll('.titleline > a')).map(a => a.textContent)
);
console.log(stories);

await browser.disconnect();
```

#### Playwright Connection
```javascript
import { chromium } from 'playwright-core';

const browser = await chromium.connectOverCDP('ws://127.0.0.1:9222');
const page = await browser.newContext().then(ctx => ctx.newPage());

await page.goto('https://example.com');
console.log(await page.title());

await browser.close();
```

> [!IMPORTANT]
> By default, `obscura serve` binds strictly to IPv4 loopback (`127.0.0.1`). If binding to external interfaces (`--host 0.0.0.0`), you must supply an authorization secret via `OBSCURA_CDP_TOKEN`.

*See [docs/Connect-Puppeteer-or-Playwright.md](docs/Connect-Puppeteer-or-Playwright.md) for advanced CDP methods and request interception.*

---

### 4. SDKs (Python & TypeScript)

Obscura provides native client libraries within the repository for programmatic control.

> [!NOTE]
> The Python and TypeScript SDKs are currently provided as in-repository packages and are not yet published to public PyPI or npm registries. Install them locally from the repository:

#### Python SDK (`sdk/python`)
```bash
# Install local package in editable mode
pip install -e sdk/python
```

```python
import asyncio
from obscura_next.client import AsyncObscuraClient

async def main():
    async with AsyncObscuraClient() as client:
        result = await client.navigate("https://example.com")
        print(f"Title: {result.title}")
        print(await client.evaluate("document.title"))

asyncio.run(main())
```

#### TypeScript SDK (`sdk/typescript`)
```bash
# Install local package
npm install ./sdk/typescript
```

```typescript
import { ObscuraClient } from '@obscura-next/client';

const client = new ObscuraClient();
const session = await client.createSession('https://example.com');
console.log(await session.evaluate('document.title'));
await client.close();
```

*See [docs/sdk/PYTHON.md](docs/sdk/PYTHON.md) and [docs/sdk/TYPESCRIPT.md](docs/sdk/TYPESCRIPT.md) for SDK references.*

---

## Rendering & Engine Architecture

Obscura's native layout and paint engine translates the DOM tree into styled visual elements without relying on Blink, WebKit, or Gecko:

- **CSS & Layout Support:** Handles Block, Inline, Flexbox, CSS Grid, positioning (`relative`, `absolute`, `fixed`, `sticky`), transforms, typography, images, and inline SVG elements.
- **Visual Capture:** Supports high-resolution viewport screenshots, full-page scrolling captures (`--screenshot`), and vector/raster PDF generation (`Page.printToPDF`).
- **Live Screencasting:** Emits real-time video frame sequences over the Chrome DevTools Protocol via `Page.startScreencast`.
- **Engine Evolution & Parity:** Obscura is designed for content extraction, agent interaction, and layout verification. Complex 3D WebGL contexts, proprietary DRM media decoders, and non-standard browser extensions are out of scope.

---

## Advanced Capabilities

### Stealth Mode & Tracker Blocking
When compiled with `--features stealth` and launched with `--stealth`, Obscura activates anti-detection protections:
- **TLS Fingerprint Emulation:** Impersonates modern desktop browser TLS client hellos and cipher suites via BoringSSL.
- **Fingerprint Randomization:** Randomizes screen metrics, canvas noise, Web Audio dynamics, and battery status while normalizing `navigator.webdriver = false`.
- **Network Tracker Blocking:** Blocks requests to over 3,500 known ad, analytics, and telemetry domains before they reach the network socket.

### SSRF Protection & Private Network Gating
To protect against Server-Side Request Forgery (SSRF) when processing untrusted user URLs, Obscura blocks navigation and sub-resource fetches to loopback (`127.0.0.1`, `localhost`) and private subnets (`10.0.0.0/8`, `192.168.0.0/16`, `172.16.0.0/12`) by default.
To allow local intranet testing, explicitly pass `--allow-private-network` or set `OBSCURA_ALLOW_PRIVATE_NETWORK=1`.

### Multi-Process Worker Pools & Supervision
In server and parallel scraping modes, Obscura uses worker child processes (`obscura-worker`) to isolate memory and execution state:
- `--workers <N>`: Sets the concurrency pool size.
- `--worker-max-rss <MB>`: Recycles workers whose memory exceeds a target threshold.
- `--worker-max-age <SECS>`: Periodically rotates worker processes.

### Distributed Execution (Optional)
For multi-node distributed browser clusters, Obscura includes optional infrastructure components (`obscura-control-plane`, `obscura-scheduler`, `obscura-supervisor`). Local CLI, MCP, and CDP workflows run self-contained and do **not** require deploying the distributed control plane. See [docs/architecture/V0.3.0_DISTRIBUTED_CONTROL_PLANE.md](docs/architecture/V0.3.0_DISTRIBUTED_CONTROL_PLANE.md).

---

## Documentation Directory

| Guide | Description |
| :--- | :--- |
| [**Installation Guide**](docs/Installation.md) | Detailed installation instructions across all supported operating systems |
| [**CLI Reference**](docs/CLI-reference.md) | Comprehensive command-line flags, options, and exit codes |
| [**MCP Onboarding**](docs/mcp/Onboarding.md) | Setup instructions for Claude Desktop, Cursor, Zed, Windsurf, and AI clients |
| [**MCP Tool Reference**](docs/mcp/TOOLS.md) | Technical reference of all 41 Model Context Protocol tools |
| [**Puppeteer Integration**](docs/Use-with-Puppeteer.md) | Connecting Puppeteer scripts to Obscura over CDP |
| [**Playwright Integration**](docs/Use-with-Playwright.md) | Connecting Playwright scripts to Obscura over CDP |
| [**CDP Protocol Reference**](docs/Connect-Puppeteer-or-Playwright.md) | Supported Chrome DevTools Protocol domains and methods |
| [**Environment Variables**](docs/Environment-variables.md) | Configuration variables for networking, V8, and process timeouts |
| [**Build from Source**](docs/Build-from-source.md) | Prerequisites and Cargo commands to compile Obscura locally |
| [**Architecture Overview**](docs/Architecture-overview.md) | Internal engine design, V8 bindings, DOM pipeline, and layout tree |
| [**Security Model**](docs/architecture/V0.3.0_SECURITY_MODEL.md) | Process containment, SSRF prevention, and Linux kernel isolation |

---

## Community, Sponsors & License

### Cloudflare Kitesurf
<table>
  <tr>
    <td width="80" align="center">
      <a href="https://blog.cloudflare.com/kitesurf/">
        <img src="https://cdn.simpleicons.org/cloudflare/F38020" alt="Cloudflare" width="50" />
      </a>
    </td>
    <td>
      <strong>Obscura inspired Cloudflare Kitesurf’s first prototype.</strong><br>
      Cloudflare began by porting Obscura to Workers while developing its agent-first browser.<br>
      <a href="https://blog.cloudflare.com/kitesurf/">Read Cloudflare’s engineering story &rarr;</a>
    </td>
  </tr>
</table>

### Obscura Cloud
We are developing **Obscura Cloud** — a hosted browser engine platform offering managed infrastructure, residential proxy pools, and dedicated enterprise support.
- **[Join the Obscura Cloud Waitlist &rarr;](https://tally.so/r/gDWzdD)**
- **[Book a Technical Demo &rarr;](https://cal.com/obscura/quick-chat)**

### Open Source Sponsors
Obscura is supported by organizations helping us build independent, open-source browser infrastructure.

<table>
  <tr>
    <td width="200" align="center" valign="middle">
      <a href="https://go.nodemaven.com/obscurareadmeoctober" target="_blank">
        <img alt="NodeMaven" src="assets/sponsors/nodemaven4.png" width="180"/>
      </a>
    </td>
    <td valign="middle">
      <a href="https://go.nodemaven.com/obscurareadmeoctober" target="_blank"><b>NodeMaven</b></a>: High-efficiency residential and mobile proxy provider for web scraping and automation.<br><br>
      <b>Special discount codes for Obscura users:</b><br>
      &bull; <b>OBSCURA35</b> — 35% off Mobile and Residential Proxies<br>
      &bull; <b>OBSCURA40</b> — 40% off Static ISP Proxies
    </td>
  </tr>
</table>

<p align="left">
  <a href="https://vercel.com/open-source-program">
    <img alt="Vercel OSS Program" src="https://vercel.com/oss/program-badge-2026.svg" />
  </a>
</p>

### License
Obscura is open-source software licensed under the [Apache License, Version 2.0](LICENSE).
