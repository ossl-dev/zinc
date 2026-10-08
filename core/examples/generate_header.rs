use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let check = match args.as_slice() {
        [] => false,
        [arg] if arg == "--check" => true,
        _ => return Err("usage: generate_header [--check]".into()),
    };
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = crate_dir.join("../include/zinc.h");
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen.toml"))?;
    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()?;
    let mut generated = Vec::new();
    bindings.write(&mut generated);

    if check {
        if fs::read(&header)? != generated {
            return Err("include/zinc.h is stale; run cargo run -p zinc-core --example generate_header --features generate-header".into());
        }
    } else {
        fs::write(&header, generated)?;
        println!("Updated {}", header.display());
    }
    Ok(())
}
