# Security Remediation: Metrics Fail-Closed, SSRF Protection, and Redirect Hardening

## 1. Metrics Authentication: Fail-Closed Architecture

### Forensic Vulnerability Analysis (SEC-02)
Prior to remediation, `crates/obscura-telemetry/src/lib.rs` served internal Prometheus metrics over unencrypted, unauthenticated TCP whenever `OBSCURA_METRICS_TOKEN` was absent or empty:
```rust
// VULNERABLE HISTORICAL CODE:
if let Ok(expected) = std::env::var("OBSCURA_METRICS_TOKEN") {
    if !expected.is_empty() {
        // Only checked if env var existed and was non-empty!
    }
}
// If unset or empty, silently fell through and served raw metrics!
```
Distributed services (`obscura-control-plane` on `0.0.0.0:9091`, `obscura-scheduler` on `0.0.0.0:9092`, and `obscura-worker-agent` on `0.0.0.0:9093`) were binding to `0.0.0.0` without declaring `OBSCURA_METRICS_TOKEN` in `deploy/compose/docker-compose.yml`, exposing runtime telemetry, session metrics, and process counts to any host on the internal or adjacent network.

### Hardened Fail-Closed Implementation
We implemented explicit type-safe configuration with fail-closed semantics in `crates/obscura-telemetry/src/lib.rs`:

1. **`MetricsAuthConfig` Enum:**
   ```rust
   pub enum MetricsAuthConfig {
       Token(String),
       DevelopmentUnauthenticated,
   }
   ```
2. **`MetricsConfigError` Fail-Closed Errors:**
   - `MissingToken`: Occurs if `OBSCURA_METRICS_TOKEN` is unset in production or non-loopback listeners.
   - `EmptyToken`: Occurs if the token is empty or whitespace-only.
   - `NonLoopbackUnauthenticated(String)`: Rejects startup if unauthenticated metrics are attempted on any non-loopback address (such as `0.0.0.0` or a public IP).
3. **Loopback Gate (`is_loopback`):**
   Parses `SocketAddr` and hostname, enforcing that only `127.0.0.1`, `::1`, and `localhost` can ever be used for development mode.
4. **Constant-Time Token Comparison:**
   Authorization headers must adhere to `Authorization: Bearer <token>`. Comparisons use XOR fold across byte slices to prevent timing attacks.
5. **Probe Separation:**
   `/health` and `/healthz` endpoints remain unauthenticated so Kubernetes and Docker container healthchecks function without credentials, while `/metrics` strictly requires authorization.

### Listener Inventory & Effective Configuration

| Service Binary | Bind Address | Port | Auth Mode | Startup Behavior if Token Missing |
|---|---|---|---|---|
| `obscura-control-plane` | `0.0.0.0:9091` | 9091 | `Token` (Bearer) | **Fails startup with error** |
| `obscura-scheduler` | `0.0.0.0:9092` | 9092 | `Token` (Bearer) | **Fails startup with error** |
| `obscura-worker-agent` | `0.0.0.0:9093` | 9093 | `Token` (Bearer) | **Fails startup with error** |
| Local Dev Runner | `127.0.0.1:909x` | Ephemeral | `DevelopmentUnauthenticated` (opt-in via `OBSCURA_DEV_MODE=1`) | Allowed on loopback only |

### Verification Suite (9 Test Cases)
Nine deterministic tests in `crates/obscura-telemetry/src/lib.rs` verify all security invariants:
1. `test_metrics_auth_case1_missing_token`: Returns `MetricsConfigError::MissingToken`.
2. `test_metrics_auth_case2_empty_token`: Returns `MetricsConfigError::EmptyToken`.
3. `test_metrics_auth_case3_valid_token`: Responds `200 OK` with Prometheus text body.
4. `test_metrics_auth_case4_missing_authorization_header`: Responds `401 Unauthorized`.
5. `test_metrics_auth_case5_invalid_token`: Responds `401 Unauthorized`.
6. `test_metrics_auth_case6_incorrect_bearer_scheme`: Rejects `Basic` or non-Bearer headers with `401 Unauthorized`.
7. `test_metrics_auth_case7_development_mode`: Loopback binding succeeds without token; non-loopback (`0.0.0.0`) fails with `NonLoopbackUnauthenticated`.
8. `test_metrics_auth_case8_health_endpoints_unauthenticated`: `/health` and `/healthz` respond `200 OK` without credentials.
9. `test_metrics_auth_case9_all_listeners_effective_configuration`: Asserts that `0.0.0.0:9091`, `9092`, and `9093` enforce tokens and reject unauthenticated development mode.

---

## 2. SSRF & DNS Rebinding Trace Analysis

### `SsrfGuardResolver` Architecture
In `crates/obscura-net/src/client.rs`:
- Custom DNS resolution (`reqwest::dns::Resolve` / `wreq`) is delegated to `SsrfGuardResolver`.
- On resolution, `tokio::net::lookup_host` resolves the hostname to a list of `SocketAddr`s.
- Every resolved `SocketAddr` is checked against `is_forbidden_ip`:
  - `0.0.0.0/8`, `10.0.0.0/8`, `100.64.0.0/10` (CGNAT), `127.0.0.0/8` (Loopback), `169.254.0.0/16` (Link-Local / AWS Metadata), `172.16.0.0/12`, `192.168.0.0/16`, `198.18.0.0/15` (Benchmarking).
  - IPv6 loopback (`::1`), IPv4-mapped IPv6 (`::ffff:127.0.0.1`, `::ffff:169.254.169.254`), ULA (`fc00::/7`), Link-local (`fe80::/10`), 6to4 (`2002::/16`), NAT64 (`64:ff9b::/96`), and Teredo (`2001:0::/32`).
- If **any** returned IP address is forbidden, the entire DNS resolution fails closed with `SsrfBlocked`.

### Prevention of DNS Rebinding (TOCTOU)
1. **Pre-flight & Connect Pinning:** The HTTP clients resolve DNS via `SsrfGuardResolver`, verifying the destination IP before socket connection. The connection is made to the exact resolved IP while passing the original hostname for SNI and HTTP `Host` header validation.
2. **Hop-by-Hop Redirect Re-Validation:** Redirects are never followed automatically by the underlying transport. Both reqwest and wreq clients use `redirect::Policy::none()`. Every redirect is processed in a manual loop:
   - Location header is parsed and checked.
   - DNS resolution and SSRF checks are re-executed on the destination host for each hop.
   - Cross-origin redirects strip sensitive `Authorization` headers.

---

## 3. Redirect Scheme & `file://` Origin Validation

### Vulnerability Analysis
In `client.rs` (line 1703) and `wreq_client.rs` (line 519), redirects invoked:
```rust
// HISTORICAL FLAW:
validate_url(&next_url, self.allow_private_network, self.allow_file_access())?;
```
When `allow_file_access` was enabled on the client (e.g. for local test harness or top-level file loading), an HTTP or HTTPS server could return:
```http
HTTP/1.1 302 Found
Location: file:///etc/passwd
```
Because `allow_file_access` was true, `validate_url` permitted the transition from a remote network origin (`http://`) to a local filesystem origin (`file://`), bypassing origin separation.

### Remediated Implementation
In both `crates/obscura-net/src/client.rs` and `crates/obscura-net/src/wreq_client.rs`:
```rust
if next_url.scheme() != "http" && next_url.scheme() != "https" {
    return Err(ObscuraNetError::Network(format!(
        "Redirect to non-HTTP/HTTPS scheme '{}' is prohibited",
        next_url.scheme()
    )));
}
validate_url(
    &next_url,
    self.allow_private_network,
    false, // Redirects NEVER permit file:// access regardless of client settings
)?;
```

### Empirical Regression Test Results
Five new regression tests in `client::redirect_validation_tests`:
- `redirect_to_file_scheme_is_rejected_even_with_allow_file_access`: **PASSED**
- `redirect_to_data_scheme_is_rejected`: **PASSED**
- `redirect_to_javascript_scheme_is_rejected`: **PASSED**
- `relative_redirect_resolves_and_follows_http`: **PASSED**
- `redirect_chain_rejects_on_second_hop_to_file`: **PASSED**

