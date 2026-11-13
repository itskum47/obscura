# Obscura Next Engineering Remediation Changelog

This changelog records all modifications implemented during the substantive forensic engineering remediation pass on `main`.

---

## 1. Modified Source Files

### 1. `crates/obscura-telemetry/src/lib.rs`
- **Objective:** Fix SEC-02 (fail-closed metrics authentication).
- **Changes:**
  - Introduced `MetricsAuthConfig` (`Token(String)` and `DevelopmentUnauthenticated`).
  - Introduced `MetricsConfigError` (`MissingToken`, `EmptyToken`, `NonLoopbackUnauthenticated`, `InvalidBindAddress`).
  - Added loopback detection helper `is_loopback(bind_addr: &str) -> bool`.
  - Refactored `MetricsServerConfig` with `new_token`, `new_development`, and `from_env`.
  - Updated `MetricsServer::spawn` and `handle_http_connection` to enforce Bearer token constant-time comparison on `/metrics` while preserving unauthenticated health checks on `/health` and `/healthz`.
  - Added 9 unit tests covering all edge cases (missing token, empty token, valid token, missing header, invalid token, incorrect scheme, dev loopback vs non-loopback, health endpoint, listener configs).
- **Verification:** `cargo test -p obscura-telemetry` (12 passed, 0 failed, exit code 0).

### 2. `crates/obscura-control-plane/src/bin/obscura-control-plane.rs`
- **Objective:** Fail closed if metrics authentication is unconfigured on startup.
- **Changes:** Changed `MetricsServer::new("0.0.0.0:9091", metrics)` to `MetricsServer::from_env("0.0.0.0:9091", metrics)?` and propagated error.
- **Verification:** `cargo check -p obscura-control-plane` (exit code 0).

### 3. `crates/obscura-scheduler/src/bin/obscura-scheduler.rs`
- **Objective:** Fail closed if metrics authentication is unconfigured on startup.
- **Changes:** Changed `MetricsServer::new("0.0.0.0:9092", ...)` to `MetricsServer::from_env("0.0.0.0:9092", ...)?` and propagated error.
- **Verification:** `cargo check -p obscura-scheduler` (exit code 0).

### 4. `crates/obscura-worker-agent/src/main.rs`
- **Objective:** Fail closed if metrics authentication is unconfigured on startup.
- **Changes:** Changed `MetricsServer::new("0.0.0.0:9093", ...)` to `MetricsServer::from_env("0.0.0.0:9093", ...).expect("metrics server configuration")`.
- **Verification:** `cargo check -p obscura-worker-agent` (exit code 0).

### 5. `deploy/compose/docker-compose.yml`
- **Objective:** Supply mandatory metrics tokens to distributed services in Compose.
- **Changes:** Added `OBSCURA_METRICS_TOKEN: ${OBSCURA_METRICS_TOKEN:?set metrics token}` under `control-plane`, `scheduler`, and `worker-agent`.
- **Verification:** Verified YAML syntax and environment variable references.

### 6. `.env.example`
- **Objective:** Document mandatory metrics authentication in production and loopback dev mode.
- **Changes:** Added explicit documentation for `OBSCURA_METRICS_TOKEN` and `OBSCURA_DEV_MODE=1`.
- **Verification:** Inspected file syntax.

### 7. `SECURITY.md`
- **Objective:** Update security contract to declare `/metrics` fail-closed.
- **Changes:** Updated configuration security section to document mandatory `OBSCURA_METRICS_TOKEN`.
- **Verification:** Inspected markdown.

### 8. `crates/obscura-net/src/client.rs`
- **Objective:** Prohibit `file://` and non-HTTP/HTTPS schemes on redirect hops.
- **Changes:**
  - Enforced `if next_url.scheme() != "http" && next_url.scheme() != "https"` on every redirect hop.
  - Passed `allow_file_access = false` to `validate_url` for all redirect hops.
  - Added `mod redirect_validation_tests` with 5 regression tests covering file redirects, data redirects, javascript redirects, relative redirects, and redirect chains.
- **Verification:** `cargo test -p obscura-net --lib redirect_validation_tests` (5 passed, exit code 0); `CI=1 cargo nextest run -p obscura-net` (140 passed, 0 failed, exit code 0).

### 9. `crates/obscura-net/src/wreq_client.rs`
- **Objective:** Prohibit `file://` and non-HTTP/HTTPS schemes on redirect hops in wreq transport.
- **Changes:** Applied identical scheme validation and `allow_file_access = false` on redirect hops.
- **Verification:** `cargo check -p obscura-net` (exit code 0).

### 10. `crates/obscura-render/src/style.rs`
- **Objective:** Defensive check for raw pointer cast in `resolve_grid_calc`.
- **Changes:** Added `if value.is_null() { return 0.0; }` before raw pointer dereference.
- **Verification:** `cargo check -p obscura-render` (exit code 0).

### 11. Workspace Manifests (`Cargo.toml` and 8 Member `Cargo.toml`s)
- **Objective:** Reconcile workspace package versioning across all crates.
- **Changes:**
  - `Cargo.toml`: Updated `[workspace.package] version = "0.3.0"`.
  - Member manifests switched from hardcoded `0.2.0` / `0.3.0` to `version.workspace = true`:
    - `crates/obscura-telemetry/Cargo.toml`
    - `crates/obscura-isolation/Cargo.toml`
    - `crates/obscura-worker/Cargo.toml`
    - `crates/obscura-supervisor/Cargo.toml`
    - `crates/obscura-agent/Cargo.toml`
    - `crates/obscura-control-plane/Cargo.toml`
    - `crates/obscura-scheduler/Cargo.toml`
    - `crates/obscura-worker-agent/Cargo.toml`
  - Retained `crates/obscura/Cargo.toml` at explicit `0.1.0` (independent embedding facade SDK).
  - Automatically updated `Cargo.lock`.
- **Verification:** `cargo metadata --no-deps` (17 crates at 0.3.0, 1 facade at 0.1.0); `cargo check --workspace` (exit code 0).

### 12. `.github/workflows/obscura-next-ci.yml`
- **Objective:** Restore meaningful CI coverage and enforce dependency security.
- **Changes:**
  - Added `workspace-check` job: runs `cargo check --workspace --all-targets` and `cargo deny check advisories bans licenses sources`.
  - Expanded `test` (unit tests) job from 7 crates to all 14 non-service crates.
  - Added `service-tests` job: executes PostgreSQL- and NATS-dependent integration tests for `obscura-control-plane` and `obscura-scheduler`.
  - Preserved `f5-adversarial` job, obstacle course gate, and security regression suite.
  - Added failure artifact log uploads to test jobs.
- **Verification:** Inspected workflow syntax and verified local execution.

---

## 2. Documentation Deliverables Created

- `docs/audits/remediation/BASELINE_RECONCILIATION.md`
- `docs/audits/remediation/CI_COVERAGE_MATRIX.md`
- `docs/audits/remediation/SECURITY_REMEDIATION.md`
- `docs/audits/remediation/BROWSER_NETWORK_REPRODUCTIONS.md`
- `docs/audits/remediation/REMEDIATION_CHANGELOG.md`

