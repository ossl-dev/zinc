# Zinc — Java Adapter

Zero-copy shared memory for Java via [JNA](https://github.com/java-native-access/jna). No native glue code, ships as a single JAR.

## Install

### Maven

```xml
<dependency>
    <groupId>dev.zinc</groupId>
    <artifactId>zinc-java</artifactId>
    <version>0.1.0</version>
</dependency>
```

Requires `libzinc_core.dylib` / `libzinc_core.so` on `java.library.path`.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
cd adapters/java
mvn package
```

## Usage

```java
import dev.zinc.SharedRegion;

// Process A — create
SharedRegion region = SharedRegion.create("/my-data", 4096);
var buf = region.buffer();
buf.putFloat(0, 42.0f);
region.notify();

// Process B — open
SharedRegion region2 = SharedRegion.open("/my-data");
region2.wait(5000);
var buf2 = region2.buffer();
float val = buf2.getFloat(0);
System.out.println(val); // 42.0
region2.close();
```

## API

### `SharedRegion.create(name, capacity)`
Create a new shared region.

### `SharedRegion.open(name)`
Open an existing shared region.

### `region.buffer() → ByteBuffer`
Zero-copy `ByteBuffer` backed by shared memory.

### `region.notify()`
Signal all waiters.

### `region.wait(timeoutMs) → boolean`
Block until notified.

### `region.close()`
Release the handle (implements `AutoCloseable`).

## Publish

```bash
cd adapters/java
mvn deploy -P release
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |
| Windows | ⏳ |
