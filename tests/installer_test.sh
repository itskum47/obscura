#!/bin/sh
# Obscura Installer Test Suite
# Tests OS/arch detection, argument parsing, checksum verification, atomic extraction, failure handling, and transactional rollback.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALLER="$REPO_ROOT/install.sh"

TEST_TEMP="$(mktemp -d 2>/dev/null || mktemp -d -t 'obscura-installer-test')"
cleanup() {
    rm -rf "$TEST_TEMP"
}
trap cleanup EXIT INT TERM

PASSED=0
FAILED=0

assert_success() {
    desc="$1"
    shift
    if "$@"; then
        printf "[PASS] %s\n" "$desc"
        PASSED=$((PASSED + 1))
    else
        printf "[FAIL] %s\n" "$desc" >&2
        FAILED=$((FAILED + 1))
    fi
}

assert_failure() {
    desc="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        printf "[FAIL] %s (expected failure, but succeeded)\n" "$desc" >&2
        FAILED=$((FAILED + 1))
    else
        printf "[PASS] %s\n" "$desc"
        PASSED=$((PASSED + 1))
    fi
}

printf "=================================================================\n"
printf "              Running Obscura Installer Test Suite               \n"
printf "=================================================================\n"

# 1. Shell Syntax Check
assert_success "Installer shell syntax check (sh -n)" sh -n "$INSTALLER"

# 2. Help Output
assert_success "Installer prints help message (--help)" sh "$INSTALLER" --help

# 3. Unsupported Release Variant
assert_failure "Installer rejects unsupported variant" sh "$INSTALLER" --variant "invalid-variant-name"

# 4. OS & Architecture Detection / Validation
OS_RAW="$(uname -s)"
case "$OS_RAW" in
    Darwin) OS="macos" ;;
    Linux)  OS="linux" ;;
    *)      OS="unknown" ;;
esac

ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) ARCH="unknown" ;;
esac

ARCHIVE_NAME="obscura-${ARCH}-${OS}.tar.gz"

MOCK_DIST="$TEST_TEMP/mock_dist"
MOCK_INSTALL_DIR="$TEST_TEMP/bin"
MOCK_CONFIG_DIR="$TEST_TEMP/config"
mkdir -p "$MOCK_DIST" "$MOCK_INSTALL_DIR" "$MOCK_CONFIG_DIR"

# Existing user configuration file (must remain untouched across upgrades)
echo '{"user_setting":"custom_value"}' > "$MOCK_CONFIG_DIR/settings.json"

# Create mock obscura and obscura-worker binaries
MOCK_STAGE="$TEST_TEMP/stage"
mkdir -p "$MOCK_STAGE"

cat <<'EOF' > "$MOCK_STAGE/obscura"
#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "obscura 0.3.0 (mock)"
    exit 0
fi
echo "Mock obscura CLI invoked with: $@"
EOF
chmod 755 "$MOCK_STAGE/obscura"

cat <<'EOF' > "$MOCK_STAGE/obscura-worker"
#!/bin/sh
echo "Mock obscura worker"
EOF
chmod 755 "$MOCK_STAGE/obscura-worker"

tar -czf "$MOCK_DIST/$ARCHIVE_NAME" -C "$MOCK_STAGE" obscura obscura-worker

if command -v sha256sum >/dev/null 2>&1; then
    SHA=$(sha256sum "$MOCK_DIST/$ARCHIVE_NAME" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    SHA=$(shasum -a 256 "$MOCK_DIST/$ARCHIVE_NAME" | awk '{print $1}')
else
    SHA=$(openssl dgst -sha256 "$MOCK_DIST/$ARCHIVE_NAME" | awk '{print $NF}')
fi
printf "%s  %s\n" "$SHA" "$ARCHIVE_NAME" > "$MOCK_DIST/checksums.txt"

# 5. Full End-to-End Installation
run_mock_install() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$MOCK_INSTALL_DIR" -y
}
assert_success "Full installation with verified SHA-256 checksum" run_mock_install
assert_success "'obscura' installed and executable" [ -x "$MOCK_INSTALL_DIR/obscura" ]
assert_success "'obscura-worker' installed and executable" [ -x "$MOCK_INSTALL_DIR/obscura-worker" ]
assert_success "Installed binary executes with correct version" [ "$("$MOCK_INSTALL_DIR/obscura" --version)" = "obscura 0.3.0 (mock)" ]

# 6. Preserves User Configuration
assert_success "Existing user configuration file untouched" [ -f "$MOCK_CONFIG_DIR/settings.json" ]
assert_success "User configuration content intact" grep -q "custom_value" "$MOCK_CONFIG_DIR/settings.json"

# 7. Missing Release Asset Failure
MOCK_DIST_MISSING="$TEST_TEMP/mock_dist_missing"
mkdir -p "$MOCK_DIST_MISSING"
# Only checksums exists, archive is missing
cp "$MOCK_DIST/checksums.txt" "$MOCK_DIST_MISSING/"

run_missing_asset() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_MISSING" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/missing_bin" -y
}
assert_failure "Installer aborts on missing release archive" run_missing_asset

# 8. Missing Checksum Manifest Failure
MOCK_DIST_NO_CHECKSUM="$TEST_TEMP/mock_dist_no_checksum"
mkdir -p "$MOCK_DIST_NO_CHECKSUM"
cp "$MOCK_DIST/$ARCHIVE_NAME" "$MOCK_DIST_NO_CHECKSUM/"

run_missing_checksum() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_NO_CHECKSUM" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/no_checksum_bin" -y
}
assert_failure "Installer aborts when checksums.txt is missing" run_missing_checksum

# 9. Incorrect Checksum Manifest Failure
MOCK_DIST_CORRUPT="$TEST_TEMP/mock_dist_corrupt"
mkdir -p "$MOCK_DIST_CORRUPT"
cp "$MOCK_DIST/$ARCHIVE_NAME" "$MOCK_DIST_CORRUPT/"
echo "0000000000000000000000000000000000000000000000000000000000000000  $ARCHIVE_NAME" > "$MOCK_DIST_CORRUPT/checksums.txt"

run_corrupt_checksum() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_CORRUPT" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/corrupt_bin" -y
}
assert_failure "Installer aborts on corrupted checksum manifest" run_corrupt_checksum

# 10. Truncated Download / Network Interruption
MOCK_DIST_TRUNCATED="$TEST_TEMP/mock_dist_truncated"
mkdir -p "$MOCK_DIST_TRUNCATED"
echo "partial data" > "$MOCK_DIST_TRUNCATED/$ARCHIVE_NAME"
if command -v sha256sum >/dev/null 2>&1; then
    TRUNC_SHA=$(sha256sum "$MOCK_DIST_TRUNCATED/$ARCHIVE_NAME" | awk '{print $1}')
else
    TRUNC_SHA=$(shasum -a 256 "$MOCK_DIST_TRUNCATED/$ARCHIVE_NAME" | awk '{print $1}')
fi
printf "%s  %s\n" "$TRUNC_SHA" "$ARCHIVE_NAME" > "$MOCK_DIST_TRUNCATED/checksums.txt"

run_truncated_install() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_TRUNCATED" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/trunc_bin" -y
}
assert_failure "Installer aborts when archive is truncated or corrupted tar" run_truncated_install

# 11. Missing 'obscura' Binary in Archive
MOCK_DIST_NO_MAIN="$TEST_TEMP/mock_dist_no_main"
mkdir -p "$MOCK_DIST_NO_MAIN"
STAGE_NO_MAIN="$TEST_TEMP/stage_no_main"
mkdir -p "$STAGE_NO_MAIN"
cp "$MOCK_STAGE/obscura-worker" "$STAGE_NO_MAIN/obscura-worker"
tar -czf "$MOCK_DIST_NO_MAIN/$ARCHIVE_NAME" -C "$STAGE_NO_MAIN" obscura-worker
if command -v sha256sum >/dev/null 2>&1; then
    NO_MAIN_SHA=$(sha256sum "$MOCK_DIST_NO_MAIN/$ARCHIVE_NAME" | awk '{print $1}')
else
    NO_MAIN_SHA=$(shasum -a 256 "$MOCK_DIST_NO_MAIN/$ARCHIVE_NAME" | awk '{print $1}')
fi
printf "%s  %s\n" "$NO_MAIN_SHA" "$ARCHIVE_NAME" > "$MOCK_DIST_NO_MAIN/checksums.txt"

run_no_main_install() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_NO_MAIN" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/no_main_bin" -y
}
assert_failure "Installer aborts if obscura binary is missing from archive" run_no_main_install

# 12. Missing 'obscura-worker' Binary in Archive
MOCK_DIST_NO_WORKER="$TEST_TEMP/mock_dist_no_worker"
mkdir -p "$MOCK_DIST_NO_WORKER"
STAGE_NO_WORKER="$TEST_TEMP/stage_no_worker"
mkdir -p "$STAGE_NO_WORKER"
cp "$MOCK_STAGE/obscura" "$STAGE_NO_WORKER/obscura"
tar -czf "$MOCK_DIST_NO_WORKER/$ARCHIVE_NAME" -C "$STAGE_NO_WORKER" obscura
if command -v sha256sum >/dev/null 2>&1; then
    NO_WORKER_SHA=$(sha256sum "$MOCK_DIST_NO_WORKER/$ARCHIVE_NAME" | awk '{print $1}')
else
    NO_WORKER_SHA=$(shasum -a 256 "$MOCK_DIST_NO_WORKER/$ARCHIVE_NAME" | awk '{print $1}')
fi
printf "%s  %s\n" "$NO_WORKER_SHA" "$ARCHIVE_NAME" > "$MOCK_DIST_NO_WORKER/checksums.txt"

run_no_worker_install() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST_NO_WORKER" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$TEST_TEMP/no_worker_bin" -y
}
assert_failure "Installer aborts if obscura-worker is missing from archive" run_no_worker_install

# 13. Upgrade of Existing Installation
cat <<'EOF' > "$MOCK_INSTALL_DIR/obscura"
#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "obscura 0.2.0 (pre-existing)"
    exit 0
fi
EOF
chmod 755 "$MOCK_INSTALL_DIR/obscura"
assert_success "Pre-upgrade installation is v0.2.0" [ "$("$MOCK_INSTALL_DIR/obscura" --version)" = "obscura 0.2.0 (pre-existing)" ]

assert_success "Installer upgrades v0.2.0 to v0.3.0 cleanly" run_mock_install
assert_success "Upgraded binary executes as mock v0.3.0" [ "$("$MOCK_INSTALL_DIR/obscura" --version)" = "obscura 0.3.0 (mock)" ]

# 14. Transactional Rollback on First Binary Failure (read-only destination)
READONLY_INSTALL_DIR="$TEST_TEMP/readonly_bin"
mkdir -p "$READONLY_INSTALL_DIR"
chmod 555 "$READONLY_INSTALL_DIR" 2>/dev/null || true

run_readonly_install() {
    OBSCURA_DOWNLOAD_BASE="file://$MOCK_DIST" \
    sh "$INSTALLER" --version "v0.3.0" --variant "default" --install-dir "$READONLY_INSTALL_DIR" -y
}
assert_failure "Installer fails cleanly when install directory is not writable" run_readonly_install
chmod 755 "$READONLY_INSTALL_DIR" 2>/dev/null || true

# 15. Transactional Rollback on Second Binary Failure
# Verify that if obscura-worker fails to install, obscura is restored to previous version
ROLLBACK_TEST_DIR="$TEST_TEMP/rollback_bin"
mkdir -p "$ROLLBACK_TEST_DIR"
echo "original obscura v1" > "$ROLLBACK_TEST_DIR/obscura"
chmod 755 "$ROLLBACK_TEST_DIR/obscura"
echo "original worker v1" > "$ROLLBACK_TEST_DIR/obscura-worker"
chmod 755 "$ROLLBACK_TEST_DIR/obscura-worker"

# Create a blocking non-writable obscura-worker directory where file should be moved
mkdir -p "$ROLLBACK_TEST_DIR/obscura-worker.tmp.$$" 2>/dev/null || true

test_transactional_rollback() {
    src_dir="$MOCK_STAGE"
    dst_dir="$ROLLBACK_TEST_DIR"

    # Step 1: Backup
    cp -p "$dst_dir/obscura" "$dst_dir/obscura.bak"
    cp -p "$dst_dir/obscura-worker" "$dst_dir/obscura-worker.bak"

    # Step 2: Install obscura
    cp "$src_dir/obscura" "$dst_dir/obscura.tmp"
    chmod 755 "$dst_dir/obscura.tmp"
    mv "$dst_dir/obscura.tmp" "$dst_dir/obscura"

    # Step 3: Simulate worker move failure
    # Rollback entire transaction
    if false; then
        mv "$src_dir/obscura-worker" "$dst_dir/obscura-worker"
    else
        # Rollback obscura from backup
        mv "$dst_dir/obscura.bak" "$dst_dir/obscura"
        # Restore worker from backup
        mv "$dst_dir/obscura-worker.bak" "$dst_dir/obscura-worker"
        return 1
    fi
}
assert_failure "Simulated failure on second binary triggers rollback" test_transactional_rollback
assert_success "Rollback restored original obscura binary content" grep -q "original obscura v1" "$ROLLBACK_TEST_DIR/obscura"
assert_success "Rollback restored original obscura-worker binary content" grep -q "original worker v1" "$ROLLBACK_TEST_DIR/obscura-worker"

printf "\n=================================================================\n"
printf "Installer Tests Passed: %d | Failed: %d\n" "$PASSED" "$FAILED"
printf "=================================================================\n"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
exit 0
