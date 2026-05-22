# Zinc — C# Adapter

Zero-copy shared memory for C# via P/Invoke. Uses `unsafe` and `Span<byte>` for direct memory access.

## Install

### NuGet

```xml
<PackageReference Include="Zinc" Version="0.1.0" />
```

Or via CLI:

```bash
dotnet add package Zinc
```

Requires `libzinc_core.dylib` / `libzinc_core.so` in the working directory or on the system library path.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
cd adapters/csharp
dotnet build
```

## Usage

```csharp
using Zinc;

// Process A — create
var region = SharedRegion.Create("/my-data", 4096);
var span = region.Bytes();
BitConverter.TryWriteBytes(span, 42.0f);
region.Notify();

// Process B — open
var region2 = SharedRegion.Open("/my-data");
region2.Wait(5000);
var span2 = region2.Bytes();
float val = BitConverter.ToSingle(span2);
Console.WriteLine(val); // 42.0
region2.Dispose();
```

## API

### `SharedRegion.Create(name, capacity) → SharedRegion`
Create a new shared region.

### `SharedRegion.Open(name) → SharedRegion`
Open an existing shared region.

### `region.Bytes() → Span<byte>`
Zero-copy span over shared memory.

### `region.Notify()`
Signal all waiters.

### `region.Wait(timeoutMs=1000) → bool`
Block until notified.

### `region.Dispose()`
Release the handle (implements `IDisposable`).

## Publish

```bash
cd adapters/csharp
dotnet pack -c Release
dotnet nuget push bin/Release/Zinc.0.1.0.nupkg
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |
| Windows | ⏳ |
