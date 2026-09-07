use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") => println!("{}", env!("CARGO_PKG_VERSION")),
        Some("build") => {
            let status = Command::new("cargo")
                .current_dir(root())
                .args(["build", "-p", "rotor-desktop", "--release", "--locked"])
                .args(&args[1..])
                .status()?;
            if !status.success() {
                return Err("native build failed".into());
            }
        }
        _ => return Err("usage: cargo run -p xtask -- version | build [cargo options]".into()),
    }
    Ok(())
}
