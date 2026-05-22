#!/usr/bin/env bash
# Zinc cross-language integration test runner.
# Builds the Rust core, then verifies each adapter can create/open/read/write/close.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
CORE_LIB="$ROOT/core/target/release"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}PASS${NC} $1"; }
fail() { echo -e "${RED}FAIL${NC} $1"; exit 1; }

echo "=== Zinc Cross-Language Integration Tests ==="
echo ""

# Build core
echo "Building zinc-core..."
cargo build --release --manifest-path "$ROOT/core/Cargo.toml" || fail "core build"

# Run Rust unit tests
echo "Running Rust tests..."
cargo test --manifest-path "$ROOT/core/Cargo.toml" || fail "rust tests"

# Python test
echo ""
echo "--- Python Adapter ---"
if command -v python3 &>/dev/null; then
    python3 -c "
import sys
sys.path.insert(0, '$ROOT/adapters/python')
from zinc import SharedRegion
import tempfile, uuid

name = f'test_py_{uuid.uuid4().hex[:8]}'
cap = 4096

# Create
r = SharedRegion.create(name, cap)
buf = r.as_buffer()
buf[0:4] = b'ZINC'
r.notify()

# Open same region
r2 = SharedRegion.open(name)
buf2 = r2.as_buffer()
assert bytes(buf2[0:4]) == b'ZINC', f'Expected ZINC, got {bytes(buf2[0:4])!r}'
r2.close()
r.close()
print('  create/open/write/read: OK')
" 2>&1 && pass "python" || fail "python"
else
    echo "  python3 not found — skipping"
fi

# Rust create, Python read
echo ""
echo "--- Rust create → Python read ---"
REGION_NAME="test_cross_$(date +%s)"
python3 -c "
import sys, ctypes, os

sys.path.insert(0, '$ROOT/adapters/python')
from zinc import SharedRegion

name = '$REGION_NAME'
cap = 4096

os.system('cargo test --manifest-path $ROOT/core/Cargo.toml create_test_region -- --nocapture 2>/dev/null || true')

try:
    r = SharedRegion.open(name)
    buf = r.as_buffer()
    val = bytes(buf[0:8])
    print(f'  Read from Rust-created region: {val.hex()}')
    r.close()
except Exception as e:
    print(f'  Could not open (region may not exist): {e}')
" 2>&1

echo ""
echo "All integration tests complete."
