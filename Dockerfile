FROM rust:1-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        curl \
        ca-certificates \
        perl \
        make \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation by copying manifests first
COPY Cargo.toml Cargo.lock ./
COPY vendor/ vendor/
COPY crates/obscura-dom/Cargo.toml          crates/obscura-dom/Cargo.toml
COPY crates/obscura-ssrf/Cargo.toml         crates/obscura-ssrf/Cargo.toml
COPY crates/obscura-net/Cargo.toml          crates/obscura-net/Cargo.toml
COPY crates/obscura-browser/Cargo.toml      crates/obscura-browser/Cargo.toml
COPY crates/obscura-cdp/Cargo.toml          crates/obscura-cdp/Cargo.toml
COPY crates/obscura-js/Cargo.toml           crates/obscura-js/Cargo.toml
COPY crates/obscura-mcp/Cargo.toml          crates/obscura-mcp/Cargo.toml
COPY crates/obscura-render/Cargo.toml       crates/obscura-render/Cargo.toml
COPY crates/obscura-cli/Cargo.toml          crates/obscura-cli/Cargo.toml
COPY crates/obscura/Cargo.toml              crates/obscura/Cargo.toml
COPY crates/obscura-telemetry/Cargo.toml    crates/obscura-telemetry/Cargo.toml
COPY crates/obscura-isolation/Cargo.toml    crates/obscura-isolation/Cargo.toml
COPY crates/obscura-worker/Cargo.toml       crates/obscura-worker/Cargo.toml
COPY crates/obscura-supervisor/Cargo.toml   crates/obscura-supervisor/Cargo.toml
COPY crates/obscura-agent/Cargo.toml        crates/obscura-agent/Cargo.toml
COPY crates/obscura-control-plane/Cargo.toml crates/obscura-control-plane/Cargo.toml
COPY crates/obscura-scheduler/Cargo.toml    crates/obscura-scheduler/Cargo.toml
COPY crates/obscura-worker-agent/Cargo.toml crates/obscura-worker-agent/Cargo.toml

# Copy build scripts and bootstrap assets required at build-time
COPY crates/obscura-cli/build.rs            crates/obscura-cli/build.rs
COPY crates/obscura-mcp/build.rs            crates/obscura-mcp/build.rs
COPY crates/obscura-js/build.rs             crates/obscura-js/build.rs
COPY crates/obscura-js/js/                  crates/obscura-js/js/

# Create stub src files so cargo can resolve the dependency graph
RUN for crate in obscura-dom obscura-ssrf obscura-net obscura-browser obscura-cdp obscura-js obscura-mcp obscura-render obscura obscura-telemetry obscura-isolation obscura-worker obscura-supervisor obscura-agent obscura-control-plane obscura-scheduler; do \
        mkdir -p crates/$crate/src && echo "// stub" > crates/$crate/src/lib.rs; \
    done && \
    mkdir -p crates/obscura-cli/src && \
    echo "fn main() {}" > crates/obscura-cli/src/main.rs && \
    echo "fn main() {}" > crates/obscura-cli/src/worker.rs && \
    mkdir -p crates/obscura-worker/src/bin && \
    echo "fn main() {}" > crates/obscura-worker/src/bin/worker_main.rs && \
    echo "fn main() {}" > crates/obscura-worker/src/bin/obscura_worker.rs && \
    mkdir -p crates/obscura-worker-agent/src && \
    echo "fn main() {}" > crates/obscura-worker-agent/src/main.rs && \
    mkdir -p crates/obscura-render/src/bin && \
    echo "fn main() {}" > crates/obscura-render/src/bin/paint_file.rs

RUN cargo build --release --features render -p obscura-cli --bin obscura -p obscura-worker --bin obscura-worker 2>/dev/null || true

ARG OBSCURA_VERSION

# Copy real sources and build
COPY crates/ crates/
RUN echo "Building Obscura version ${OBSCURA_VERSION:-from Cargo.toml}" && \
    find crates -name "*.rs" -exec touch {} + 2>/dev/null || true && \
    cargo build --release --features render -p obscura-cli --bin obscura -p obscura-worker --bin obscura-worker

# ---

# distroless/cc: glibc + libgcc + CA certs only — no shell, no package manager.
#
# `:nonroot` runs as uid/gid 65532 instead of root. Obscura executes untrusted
# page JavaScript in-process through V8, so a V8 exploit lands with the
# process's privileges; there is no reason for those to be root's. The image
# needs no privileged operation: it binds an unprivileged port, reads the CA
# bundle, and writes only to the storage dir and a temp dir.
#
# The tag is deliberately not pinned to a digest. distroless is rebuilt often
# with base-layer security patches, and tracking the tag picks those up; a
# digest pin would freeze them until someone remembers to bump it, which for a
# *base* image trades a real ongoing risk for a theoretical one.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/obscura /obscura
COPY --from=builder /build/target/release/obscura-worker /obscura-worker

EXPOSE 9222

# Bind to 0.0.0.0 *inside the container*: a container-loopback bind is
# unreachable through `-p`, so this is required for the port to work at all.
# It is not a licence to expose the port. Non-loopback binds require
# OBSCURA_CDP_TOKEN; publish to host loopback as an additional boundary.
# The native binary still defaults to 127.0.0.1.
#
ENTRYPOINT ["/obscura"]
CMD ["serve", "--port", "9222", "--host", "0.0.0.0"]
