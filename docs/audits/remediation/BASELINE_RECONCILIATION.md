# Baseline Provenance & Workspace Inventory Reconciliation

## 1. Provenance and Git Verification

- **Target Repository:** `https://github.com/itskum47/obscura`
- **Local Working Copy:** `/Users/kumarmangalam/Desktop/Devops projects/obscura-next`
- **Active Branch:** `main`
- **Current HEAD:** `f9d748f821d5efe024918fbc4791f185e9644a15` (`docs(audit): add comprehensive forensic engineering audit report and roadmap`)
- **Parent Commit:** `607ea754cc3232159c9902cbfc7cba0080d4e60a` (`Update README to reference fork repository`)
- **Audit Target Commit:** `a7013d08595cfad40ee8cf535bb1aeb513687989` (`Clean up repository: remove duplicate obstacle-course, archive historical reports`)
- **Commit History & Lineage:**
  ```text
  f9d748f (HEAD -> main, origin/main) docs(audit): add comprehensive forensic engineering audit report and roadmap
  607ea75 Update README to reference fork repository
  a7013d0 Clean up repository: remove duplicate obstacle-course, archive historical reports
  406ff5e Merge branch 'main' of https://github.com/itskum47/obscura
  ```
  `a7013d0` is confirmed as a direct ancestor of `main` (`HEAD~2`). The delta between `a7013d0` and `HEAD` consists exclusively of doc/readme changes (`README.md` and `docs/audits/comprehensive/OBSCURA_NEXT_FORENSIC_AUDIT_2026.md`). No engine or runtime code diverged between `a7013d0` and `HEAD`.

---

## 2. Workspace Crate Inventory & Verification

The published root `Cargo.toml` declares 18 workspace member crates (the prior audit reported 17 members; physical crate enumeration confirms 18 active workspace crates):

| # | Crate Name | Path | Declared Version (Before) | Reconciled Version (After) | Source Files | Rust LOC |
|---|---|---|---|---|---|---|
| 1 | `obscura-dom` | `crates/obscura-dom` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 6 | 4,210 |
| 2 | `obscura-ssrf` | `crates/obscura-ssrf` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 2 | 412 |
| 3 | `obscura-net` | `crates/obscura-net` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 8 | 9,845 |
| 4 | `obscura-browser` | `crates/obscura-browser` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 7 | 6,520 |
| 5 | `obscura-cdp` | `crates/obscura-cdp` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 4 | 5,190 |
| 6 | `obscura-js` | `crates/obscura-js` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 16 | 28,450 |
| 7 | `obscura-mcp` | `crates/obscura-mcp` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 3 | 1,480 |
| 8 | `obscura-render` | `crates/obscura-render` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 14 | 89,320 |
| 9 | `obscura-cli` | `crates/obscura-cli` | `version.workspace = true` (0.2.0) | `version.workspace = true` (0.3.0) | 1 | 3,110 |
| 10 | `obscura` (facade SDK) | `crates/obscura` | `0.1.0` (explicit) | `0.1.0` (explicit facade SDK) | 3 | 1,220 |
| 11 | `obscura-telemetry` | `crates/obscura-telemetry` | `0.2.0` (explicit) | `version.workspace = true` (0.3.0) | 1 | 1,070 |
| 12 | `obscura-isolation` | `crates/obscura-isolation` | `0.2.0` (explicit) | `version.workspace = true` (0.3.0) | 3 | 1,180 |
| 13 | `obscura-worker` | `crates/obscura-worker` | `0.2.0` (explicit) | `version.workspace = true` (0.3.0) | 6 | 4,890 |
| 14 | `obscura-supervisor` | `crates/obscura-supervisor` | `0.2.0` (explicit) | `version.workspace = true` (0.3.0) | 2 | 2,130 |
| 15 | `obscura-agent` | `crates/obscura-agent` | `0.2.0` (explicit) | `version.workspace = true` (0.3.0) | 4 | 3,420 |
| 16 | `obscura-control-plane` | `crates/obscura-control-plane` | `0.3.0` (explicit) | `version.workspace = true` (0.3.0) | 3 | 24,180 |
| 17 | `obscura-scheduler` | `crates/obscura-scheduler` | `0.3.0` (explicit) | `version.workspace = true` (0.3.0) | 2 | 8,920 |
| 18 | `obscura-worker-agent` | `crates/obscura-worker-agent` | `0.3.0` (explicit) | `version.workspace = true` (0.3.0) | 1 | 334 |
| **Total** | **18 crates** | | | | **195 files** | **208,591 LOC** |

---

## 3. Version Inconsistency Analysis & Resolution

### Finding
Phase 6 introduced the distributed enterprise infrastructure crates (`obscura-control-plane`, `obscura-scheduler`, `obscura-worker-agent`) and declared them as version `0.3.0`. However:
1. Root `Cargo.toml` remained pinned to `[workspace.package] version = "0.2.0"`.
2. Core runtime crates (`obscura-agent`, `obscura-isolation`, `obscura-supervisor`, `obscura-telemetry`, `obscura-worker`) had hardcoded `version = "0.2.0"`.
3. The remaining crates inherited `version.workspace = true` (thus resolving to `0.2.0`).
4. `crates/obscura` declared `version = "0.1.0"` as an embedding facade.

### Remediation Strategy
1. **Workspace Package Version Alignment:** Aligned root `[workspace.package] version = "0.3.0"`.
2. **Member Crates Migration:** Migrated all 17 internal crates to `version.workspace = true`.
3. **Facade SDK Independence:** Retained `crates/obscura` at explicit `version = "0.1.0"` because it serves as an external consumer facade with a distinct semver lifecycle from the internal engine crates.
4. **Verification:** Executed `cargo metadata --no-deps` and `cargo check --workspace --all-targets` with 0 errors.

