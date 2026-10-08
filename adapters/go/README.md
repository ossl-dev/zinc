# Zinc Go adapter

cgo links against the core in target/release. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
export LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$PWD/target/release:${DYLD_LIBRARY_PATH:-}"
cd adapters/go
go vet ./...
go test ./...
```

Import from your source checkout:

```go
import "zinc"
```

For another module, add `replace zinc => /path/to/zinc/adapters/go` to go.mod and run `go get zinc`. `Bytes()` borrows the mapping: do not close while using the slice. `Close()` waits for active methods and releases the handle once, even when a `SharedRegion` has been copied. Copies share their position and close state. `Wait()` returns false on failure; `TryWait()` checks without blocking.

`Read`, `Write`, and `Seek` implement `io.ReadWriteSeeker`; `ReadAt` and `WriteAt` leave the position unchanged. Reads stop at capacity with `io.EOF`, and writes that do not fit return `io.ErrShortWrite`. These methods copy bytes and serialize access through the same handle. Other mappings and borrowed slices still need your synchronization protocol. Writes do not notify automatically.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/go.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).

Run `go test -run '^$' -bench BenchmarkCopyAt -benchmem` to measure paired `WriteAt`/`ReadAt` copies through one handle. This measures local copying and adapter overhead, not cross-process notification latency. The adapter caches its data view when opening the mapping, so copy operations do not cross cgo.
