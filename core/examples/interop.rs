use std::io::{self, BufRead, Write};
use zinc_core::SharedRegion;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args().nth(1).ok_or("missing region name")?;
    let page = usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) })?;
    let region = SharedRegion::create(&name, page)?;
    unsafe { std::ptr::copy_nonoverlapping(b"ZINC".as_ptr(), region.as_ptr(), 4) };
    region.notify();
    println!("ready");
    io::stdout().flush()?;
    let mut shutdown = String::new();
    io::stdin().lock().read_line(&mut shutdown)?;
    Ok(())
}
