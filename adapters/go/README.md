# Zinc — Go Adapter

Zero-copy shared memory for Go via [cgo](https://pkg.go.dev/cmd/cgo).

## Install

```bash
go get github.com/aspect-build/zinc/adapters/go
```

Requires `libzinc_core.dylib` / `libzinc_core.so` built and findable in `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH`.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
go build ./adapters/go/...
```

## Usage

```go
package main

import (
    "fmt"
    "unsafe"
    "github.com/aspect-build/zinc/adapters/go"
)

func main() {
    // Process A — create
    r, err := zinc.Create("/my-data", 4096)
    if err != nil { panic(err) }
    defer r.Close()

    data := r.Bytes()
    *(*float32)(unsafe.Pointer(&data[0])) = 42.0
    r.Notify()
}
```

```go
// Process B — open
r, err := zinc.Open("/my-data")
if err != nil { panic(err) }
defer r.Close()

r.Wait(5000)
data := r.Bytes()
val := *(*float32)(unsafe.Pointer(&data[0]))
fmt.Println(val) // 42.0
```

## API

### `zinc.Create(name string, capacity uint) (*SharedRegion, error)`
Create a new shared region.

### `zinc.Open(name string) (*SharedRegion, error)`
Open an existing shared region.

### `region.Bytes() []byte`
Zero-copy Go slice backed by shared memory.

### `region.Notify()`
Signal all waiters.

### `region.Wait(timeoutMs uint32) bool`
Block until notified.

### `region.Close()`
Release the handle.

## Testing

```bash
go test ./adapters/go/...
```

## Publish

```bash
git tag v0.1.0
git push origin v0.1.0
# Go proxy picks up the tag automatically
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |

> Windows is not supported. Zinc requires POSIX `shm_open` + `mmap`.
