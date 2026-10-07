use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use zinc_core::SharedRegion;

pub fn notification_roundtrip() -> (f64, usize) {
    let request_name = format!("ex_req_{}", std::process::id());
    let reply_name = format!("ex_rep_{}", std::process::id());
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
    let request = SharedRegion::create(&request_name, page).expect("create request");
    let reply = SharedRegion::create(&reply_name, page).expect("create reply");
    let worker_request = SharedRegion::open(&request_name).expect("open request");
    let worker_reply = SharedRegion::open(&reply_name).expect("open reply");
    let done = AtomicBool::new(false);
    let iters = 5_000;

    let elapsed = std::thread::scope(|scope| {
        scope.spawn(|| loop {
            worker_request.wait(5000).expect("wait request");
            if done.load(Ordering::Relaxed) {
                break;
            }
            worker_reply.notify();
        });
        for _ in 0..100 {
            request.notify();
            reply.wait(5000).expect("warmup reply");
        }
        let start = Instant::now();
        for _ in 0..iters {
            request.notify();
            reply.wait(5000).expect("wait reply");
        }
        let elapsed = start.elapsed();
        done.store(true, Ordering::Relaxed);
        request.notify();
        elapsed
    });
    (elapsed.as_secs_f64() * 1_000_000.0 / iters as f64, iters)
}
