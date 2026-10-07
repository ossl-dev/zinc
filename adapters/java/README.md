# Zinc Java adapter

JNA calls the core library. The source targets Java 17 or later. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
export LD_LIBRARY_PATH="$PWD/target/release:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$PWD/target/release:${DYLD_LIBRARY_PATH:-}"
cd adapters/java
mvn test
mvn install
```

Import from your source checkout:

```java
import dev.zinc.SharedRegion;
```

Use the locally installed dev.zinc:zinc-java:0.1.0 jar. `buffer()` returns a ByteBuffer in native byte order; keep the region open while using it. `AutoCloseable` supports try-with-resources. Notifications use `signal()`, `waitForNotification(timeoutMs)`, and `tryWait()` because Object reserves notify and wait for monitors.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/java.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
