# Zinc C# adapter

P/Invoke calls the core library. The source targets .NET 8. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
export LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$PWD/target/release:${DYLD_LIBRARY_PATH:-}"
dotnet test adapters/csharp/tests/Zinc.Tests.csproj
```

Import from your source checkout:

```csharp
using Zinc;
```

Add a project reference to `adapters/csharp/Zinc.csproj`. `Bytes()` returns a Span<byte> that borrows the mapping; keep the region open while using it. `IDisposable` supports using statements. `Wait()` returns false on timeout and throws on other errors; `TryWait()` checks without blocking.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/csharp.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
