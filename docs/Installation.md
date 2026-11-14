# Installing Obscura

Obscura is a standalone, high-performance headless browser and AI automation platform built in Rust. It requires **no Google Chrome or Chromium installation**.

---

## 1. One-Command Native Installation

The official installer automatically detects your operating system and CPU architecture, downloads the verified release archive, checks SHA-256 integrity, extracts both `obscura` and `obscura-worker`, and installs them to your user directory (`~/.local/bin` on Unix, `%LOCALAPPDATA%\Obscura\bin` on Windows).

### macOS & Linux

```bash
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh
```

*(Once custom domain DNS and edge routing are active, the shorthand `curl -fsSL https://obscura.sh/install.sh | sh` will also be supported).*

#### Installer Options
You can customize the version, variant, or target directory:

```bash
# Install specific release tag
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --version v0.3.0

# Install stealth variant with TLS impersonation
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --variant stealth

# Custom installation path
curl -fsSL https://raw.githubusercontent.com/itskum47/obscura/main/install.sh | sh -s -- --install-dir /opt/obscura/bin
```

---

### Windows (PowerShell)

Open Windows PowerShell (Run as Administrator not required):

```powershell
irm https://raw.githubusercontent.com/itskum47/obscura/main/install.ps1 | iex
```

*(Once domain routing is active: `irm https://obscura.sh/install.ps1 | iex`)*

#### PowerShell Parameters

```powershell
# Install specific version and variant
.\install.ps1 -Version "v0.3.0" -Variant "stealth" -InstallDir "C:\Tools\Obscura"
```

---

## 2. Verify Your Installation

Verify the installed binary and run the diagnostic health check:

```bash
# Check version
obscura --version

# Run full system diagnostics
obscura doctor
```

`obscura doctor` non-destructively inspects:
- **V8 JavaScript Runtime:** Verifies isolate initialization and in-memory evaluation.
- **Rendering & Layout:** Probes pure-Rust flexbox/grid layout and 2D rasterizer.
- **Worker Process:** Confirms adjacent or PATH discovery of `obscura-worker`.
- **MCP Server:** Confirms tool registration for AI agents.
- **CDP Loopback:** Checks local IPv4 loopback socket permissions.
- **Storage & Permissions:** Probes cache write access.
- **Platform Containment:** Evaluates OS sandboxing capabilities (Linux namespaces, Landlock LSM, Seccomp-BPF vs Darwin Seatbelt).

---

## 3. Release Variants

Prebuilt release archives are compiled in four configurations:

| Variant | Archive Suffix | Rendering Engine | Anti-Detect Stealth | Use Case |
| :--- | :--- | :--- | :--- | :--- |
| **Default** (Recommended) | *(none)* | ✅ Yes | ❌ Standard | Screenshots, PDF export, visual testing, web automation |
| **Stealth** | `-stealth` | ✅ Yes | ✅ TLS & fingerprinting | Scraping Cloudflare/Datadome protected sites |
| **No-Render** | `-no-render` | ❌ No | ❌ Standard | Fast headless DOM traversal, text & JSON extraction |
| **No-Render Stealth** | `-no-render-stealth` | ❌ No | ✅ TLS & fingerprinting | Minimal-memory scraping of protected endpoints |

---

## 4. Manual Download & Verification

If you prefer to download archives manually from [GitHub Releases](https://github.com/itskum47/obscura/releases):

### Linux x86_64
```bash
curl -LO https://github.com/itskum47/obscura/releases/latest/download/obscura-x86_64-linux.tar.gz
curl -LO https://github.com/itskum47/obscura/releases/latest/download/checksums.txt
sha256sum --check --ignore-missing checksums.txt
tar xzf obscura-x86_64-linux.tar.gz
./obscura doctor
```

### Linux ARM64 (aarch64)
```bash
curl -LO https://github.com/itskum47/obscura/releases/latest/download/obscura-aarch64-linux.tar.gz
curl -LO https://github.com/itskum47/obscura/releases/latest/download/checksums.txt
sha256sum --check --ignore-missing checksums.txt
tar xzf obscura-aarch64-linux.tar.gz
./obscura doctor
```

### macOS Apple Silicon (M1/M2/M3/M4)
```bash
curl -LO https://github.com/itskum47/obscura/releases/latest/download/obscura-aarch64-macos.tar.gz
curl -LO https://github.com/itskum47/obscura/releases/latest/download/checksums.txt
shasum -a 256 -c checksums.txt --ignore-missing
tar xzf obscura-aarch64-macos.tar.gz
./obscura doctor
```

### macOS Intel (x86_64)
```bash
curl -LO https://github.com/itskum47/obscura/releases/latest/download/obscura-x86_64-macos.tar.gz
curl -LO https://github.com/itskum47/obscura/releases/latest/download/checksums.txt
shasum -a 256 -c checksums.txt --ignore-missing
tar xzf obscura-x86_64-macos.tar.gz
./obscura doctor
```

### Windows (x86_64)
1. Download `obscura-x86_64-windows.zip` and `checksums.txt` from Releases.
2. In PowerShell, verify hash:
   ```powershell
   Get-FileHash .\obscura-x86_64-windows.zip -Algorithm SHA256
   ```
3. Extract files into a folder in your PATH (e.g. `C:\Users\<user>\AppData\Local\Obscura\bin`).

> [!IMPORTANT]
> Both `obscura` and `obscura-worker` binaries must remain in the same directory or accessible via your system `PATH`. The parallel `scrape` command delegates concurrent page isolation to worker child processes.

---

## 5. Docker Container

Obscura publishes container images based on Google's `distroless/cc:nonroot` with zero shell or package manager in the runtime:

```bash
docker run -d \
  --name obscura \
  -p 127.0.0.1:9222:9222 \
  itskum47/obscura
```

For production deployments with persistence and authorization tokens, see [Run in production at scale](Run-in-production-at-scale.md).

---

## 6. SDK Onboarding

### Python SDK
Install the official Python client:

```bash
pip install obscura-next
```

Basic script:
```python
import asyncio
from obscura_next.client import AsyncObscuraClient

async def main():
    async with AsyncObscuraClient() as client:
        result = await client.navigate("https://example.com")
        print(f"Title: {result.title}")
        title = await client.evaluate("document.title")
        print(f"Evaluated: {title}")

asyncio.run(main())
```

### TypeScript SDK
Install the official Node.js / TypeScript package:

```bash
npm install @obscura-next/client
```

Basic script:
```typescript
import { ObscuraClient } from '@obscura-next/client';

async function main() {
  const client = new ObscuraClient();
  const session = await client.createSession('https://example.com');
  const title = await session.evaluate('document.title');
  console.log(`Page title: ${title}`);
  await client.close();
}

main();
```

---

## 7. Building from Source

To compile Obscura locally using Cargo:

```bash
git clone https://github.com/itskum47/obscura.git
cd obscura

# Build with rendering engine (recommended)
cargo build --release -p obscura-cli --bins --features render

# Build with rendering and stealth TLS impersonation
cargo build --release -p obscura-cli --bins --features render,stealth
```

Binaries will be placed in `target/release/obscura` and `target/release/obscura-worker`.
See [Build from source](Build-from-source.md) for full prerequisites.

---

## 8. Troubleshooting

- **`cannot execute binary file: Exec format error`**: The architecture of the downloaded binary does not match your CPU. Check `uname -m` (use `aarch64` for ARM, `x86_64` for Intel/AMD).
- **`GLIBC_2.35 not found`**: Your Linux distribution is older than Ubuntu 22.04 LTS. Use the official Docker container or build from source using Rust.
- **macOS Gatekeeper warning (`cannot be opened because Apple cannot check it for malicious software`)**:
  Prebuilt macOS binaries are ad-hoc signed and not notarized by Apple. The installer preserves Gatekeeper quarantine attributes by default. To authorize:
  1. Go to **System Settings** -> **Privacy & Security** -> click **Allow Anyway** next to Obscura.
  2. Alternatively, manually clear the quarantine attribute:
     ```bash
     xattr -d com.apple.quarantine ~/.local/bin/obscura ~/.local/bin/obscura-worker
     ```
- **`Worker binary 'obscura-worker' not found`**: Ensure `obscura-worker` is placed in the exact same directory as `obscura` or is in your system `$PATH`. Run `obscura doctor` to verify worker discovery.
