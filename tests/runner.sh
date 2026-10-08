#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export LD_LIBRARY_PATH="$ROOT/target/release:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$ROOT/target/release:${DYLD_LIBRARY_PATH:-}"
PYTHON="${ZINC_PYTHON:-python3}"

cargo run -p zinc-core --example generate_header --features generate-header --locked -- --check
cargo test --workspace --all-targets --locked
cargo build --release -p zinc-core --lib --example interop --locked

if command -v "$PYTHON" >/dev/null; then
    "$PYTHON" -m pytest adapters/python/tests -q
    "$PYTHON" -m mypy --strict adapters/python/tests/typing_check.py
else
    echo "SKIP Python: runtime unavailable"
fi
if command -v go >/dev/null; then
    (cd adapters/go && go vet ./... && go test -race ./...)
else
    echo "SKIP Go: runtime unavailable"
fi
if command -v bun >/dev/null; then
    bun test adapters/bun/tests
else
    echo "SKIP Bun: runtime unavailable"
fi
if command -v deno >/dev/null; then
    deno test --unstable-ffi --allow-ffi --allow-read --config adapters/deno/deno.json adapters/deno/tests/zinc_test.ts
else
    echo "SKIP Deno: runtime unavailable"
fi
if command -v node >/dev/null && command -v npm >/dev/null; then
    (cd adapters/node && npm ci && npm run build && npm test)
else
    echo "SKIP Node: runtime unavailable"
fi
if command -v cmake >/dev/null; then
    cmake -S adapters/cpp -B target/cpp-tests
    cmake --build target/cpp-tests
    ctest --test-dir target/cpp-tests --output-on-failure
else
    echo "SKIP C++: CMake unavailable"
fi
if command -v mvn >/dev/null; then
    (cd adapters/java && mvn --batch-mode test)
else
    echo "SKIP Java: Maven unavailable"
fi
if command -v dotnet >/dev/null; then
    dotnet test adapters/csharp/tests/Zinc.Tests.csproj
else
    echo "SKIP C#: .NET SDK unavailable"
fi
