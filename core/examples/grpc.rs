#[cfg(not(windows))]
use std::io::{Read, Write};
#[cfg(not(windows))]
use std::net::{TcpListener, TcpStream};
#[cfg(not(windows))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(windows))]
use std::sync::mpsc;
#[cfg(not(windows))]
use std::sync::Arc;
#[cfg(not(windows))]
use std::thread;
#[cfg(not(windows))]
use std::time::{Duration, Instant};

#[cfg(not(windows))]
use prost::Message;

#[cfg(not(windows))]
use zinc_core::SharedRegion;

// Protobuf: single rpc Transfer(DataChunk) returns (Empty).
#[cfg(not(windows))]
#[derive(Clone, PartialEq, prost::Message)]
struct DataChunk {
    #[prost(bytes, tag = "1")]
    payload: Vec<u8>,
}

#[cfg(not(windows))]
#[derive(Clone, PartialEq, prost::Message)]
struct EmptyPayload {}

fn main() {
    #[cfg(not(windows))]
    run_benchmarks();

    #[cfg(windows)]
    {
        eprintln!("Zinc vs gRPC benchmark: not supported on Windows");
        std::process::exit(1);
    }
}

// ── Shared constants & helpers ───────────────────────────────────

#[cfg(not(windows))]
const NAME: &str = "bench_zinc_grpc";

#[cfg(not(windows))]
struct Result {
    gbps: f64,
    total_gb: f64,
}

#[cfg(not(windows))]
fn cleanup(name: &str) {
    let cname = std::ffi::CString::new(format!("/zinc_{name}")).ok();
    if let Some(cn) = cname {
        unsafe { libc::shm_unlink(cn.as_ptr()); }
    }
}

#[cfg(not(windows))]
fn create_region(name: &str, size: usize) -> zinc_core::SharedRegion {
    cleanup(name);
    SharedRegion::create(name, page_align(size)).expect("create")
}

#[cfg(not(windows))]
fn open_region(name: &str) -> zinc_core::SharedRegion {
    SharedRegion::open(name).expect("open")
}

#[cfg(not(windows))]
fn page_align(size: usize) -> usize {
    let page = page_size();
    (size + page - 1) & !(page - 1)
}

#[cfg(not(windows))]
fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

#[cfg(not(windows))]
fn fmt_size(kb: usize) -> String {
    if kb >= 1_048_576 {
        format!("{} GB", kb / 1_048_576)
    } else if kb >= 1024 && kb % 1024 == 0 {
        format!("{} MB", kb / 1024)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{} KB", kb)
    }
}

#[cfg(not(windows))]
fn pick_iters(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 100_000,
        p if p <= 65_536 => 10_000,
        p if p <= 1_048_576 => 1_000,
        _ => 100,
    }
}

#[cfg(not(windows))]
fn warmup_iters(payload: usize) -> usize {
    (pick_iters(payload) / 10).max(10)
}

#[cfg(not(windows))]
fn pick_iters_grpc(payload: usize) -> usize {
    match payload {
        p if p <= 1024 => 5_000,
        p if p <= 65_536 => 500,
        p if p <= 1_048_576 => 50,
        _ => 10,
    }
}

#[cfg(not(windows))]
fn warmup_iters_grpc(payload: usize) -> usize {
    (pick_iters_grpc(payload) / 5).max(5)
}

// ── TCP framing helpers — 4-byte LE length prefix + message ──────

#[cfg(not(windows))]
fn tcp_write_msg(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let len = data.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    if !data.is_empty() {
        stream.write_all(data)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn tcp_read_msg(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

// ── Notify/wait latency ────────────────────────────────────────

#[cfg(not(windows))]
fn bench_notify_latency() -> (f64, usize) {
    let parent = create_region(NAME, 4096);
    let child = open_region(NAME);

    let done = Arc::new(AtomicBool::new(false));
    let done_signal = done.clone();
    let handle = thread::spawn(move || {
        while !done_signal.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_micros(1));
            parent.notify();
        }
    });

    let iters = 5_000;
    let start = Instant::now();
    for _ in 0..iters {
        child.wait(5000).expect("wait");
    }
    done.store(true, Ordering::Release);
    let elapsed = start.elapsed();
    handle.join().unwrap();

    let avg = elapsed.as_secs_f64() / iters as f64 * 1_000_000.0;
    (avg, iters)
}

// ── Zinc transfer ───────────────────────────────────────────────

#[cfg(not(windows))]
fn bench_zinc_transfer(nominal: usize, aligned: usize) -> Result {
    let parent = create_region(NAME, aligned);
    let child = open_region(NAME);

    let iters = pick_iters(nominal);
    let payload = aligned;
    let warmup = warmup_iters(nominal);

    for i in 0..warmup {
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("warmup");
        std::hint::black_box(unsafe { std::ptr::read(child.as_ptr()) });
    }

    let start = Instant::now();
    for i in 0..iters {
        unsafe { std::ptr::write_bytes(parent.as_ptr(), (i % 256) as u8, payload) }
        parent.notify();
        child.wait(5000).expect("wait");
        std::hint::black_box(unsafe { std::ptr::read(child.as_ptr()) });
    }
    let elapsed = start.elapsed();
    let total_bytes = payload as f64 * iters as f64;

    drop(child);
    drop(parent);

    Result {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

// ── gRPC transfer ──────────────────────────────────────────────

#[cfg(not(windows))]
fn bench_grpc_transfer(payload: usize) -> Result {
    let (tx_port, rx_port) = mpsc::channel();

    let server = thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("grpc server bind");
        tx_port.send(listener.local_addr().unwrap().port()).expect("send port");

        let (mut stream, _peer) = listener.accept().expect("grpc server accept");
        loop {
            let data = match tcp_read_msg(&mut stream) {
                Ok(v) => v,
                Err(_) => break,
            };
            let _chunk = DataChunk::decode(&data[..]).expect("server decode");
            let mut resp = Vec::new();
            EmptyPayload {}.encode(&mut resp).expect("server encode");
            tcp_write_msg(&mut stream, &resp).expect("server write");
        }
    });

    let port = rx_port.recv().expect("recv port");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("grpc client connect");

    let chunk = DataChunk { payload: vec![0u8; payload] };
    let iters = pick_iters_grpc(payload);
    let warmup = warmup_iters_grpc(payload);

    for _ in 0..warmup {
        let mut encoded = Vec::new();
        chunk.encode(&mut encoded).expect("warmup encode");
        tcp_write_msg(&mut stream, &encoded).expect("warmup write");
        let resp = tcp_read_msg(&mut stream).expect("warmup read");
        let _empty = EmptyPayload::decode(&resp[..]).expect("warmup decode");
    }

    let start = Instant::now();
    for _ in 0..iters {
        let mut encoded = Vec::new();
        chunk.encode(&mut encoded).expect("encode");
        tcp_write_msg(&mut stream, &encoded).expect("write");
        let resp = tcp_read_msg(&mut stream).expect("read");
        let _empty = EmptyPayload::decode(&resp[..]).expect("decode");
        std::hint::black_box(());
    }
    let elapsed = start.elapsed();

    stream.shutdown(std::net::Shutdown::Write).ok();
    server.join().expect("grpc server join");
    drop(stream);

    let total_bytes = payload as f64 * iters as f64;
    Result {
        gbps: total_bytes / elapsed.as_secs_f64() / 1_000_000_000.0,
        total_gb: total_bytes / (1024.0 * 1024.0 * 1024.0),
    }
}

// ── Orchestrator ────────────────────────────────────────────────

#[cfg(not(windows))]
fn run_benchmarks() {
    let (latency_us, latency_iters) = bench_notify_latency();

    // Skip 1 GB for gRPC — protobuf serialization would be impractically slow.
    let sizes: &[usize] = &[1, 64, 1024, 10240];
    let mut rows: Vec<(usize, Result, Result)> = Vec::new();

    const SAMPLES: usize = 5;

    for &payload_kb in sizes {
        let nominal = payload_kb * 1024;
        let aligned = page_align(nominal);

        let mut zinc_best = 0.0_f64;
        let mut grpc_best = 0.0_f64;
        let mut zinc_data = 0.0_f64;
        let mut grpc_data = 0.0_f64;

        for s in 0..SAMPLES {
            let (z, g) = if s % 2 == 0 {
                (bench_zinc_transfer(nominal, aligned), bench_grpc_transfer(nominal))
            } else {
                let g = bench_grpc_transfer(nominal);
                let z = bench_zinc_transfer(nominal, aligned);
                (z, g)
            };
            if z.gbps > zinc_best { zinc_best = z.gbps; zinc_data = z.total_gb; }
            if g.gbps > grpc_best { grpc_best = g.gbps; grpc_data = g.total_gb; }
        }

        rows.push((payload_kb,
            Result { gbps: zinc_best, total_gb: zinc_data },
            Result { gbps: grpc_best, total_gb: grpc_data },
        ));
    }

    const GRN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const RST: &str = "\x1b[0m";
    const BLD: &str = "\x1b[1m";

    println!("\n\n{}══════════════════════════════════════════════════════{}", BLD, RST);
    println!("{}         Zinc vs gRPC (prost + TCP) — Throughput{}", BLD, RST);
    println!("{} Notify/wait latency: {:.1} µs avg ({} iters){}", BLD, latency_us, latency_iters, RST);
    println!("{}══════════════════════════════════════════════════════{}", BLD, RST);
    println!(" {:<8} {:>12} {:>12} {:>6} {:>10}", "Payload", "Zinc", "gRPC", "Ratio", "Data");
    println!("{0:\u{2500}^10} {0:\u{2500}^14} {0:\u{2500}^14} {0:\u{2500}^7} {0:\u{2500}^12}", "");

    for (kb, z, g) in &rows {
        let label = fmt_size(*kb);
        let ratio = z.gbps / g.gbps;
        let total = (z.total_gb + g.total_gb) / 2.0;

        println!(
            " {:<8} {}{:>10.2} GB/s{} {}{:>10.2} GB/s{} {:>5.0}x {:>8.2} GB",
            label, GRN, z.gbps, RST, RED, g.gbps, RST, ratio, total,
        );
    }

    println!("{0:\u{2500}^10} {0:\u{2500}^14} {0:\u{2500}^14} {0:\u{2500}^7} {0:\u{2500}^12}", "");
    println!("{}Zinc: zero-copy shared memory (memory-bandwidth-bound).{}", GRN, RST);
    println!("{}gRPC: protobuf serialize + TCP stack + deserialize — O(n) overhead.{}", RED, RST);
    println!(
        "{}Method: min-time (max GB/s) across {} samples, alternating order.{}",
        RST, SAMPLES, RST
    );
}
