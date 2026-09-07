//! Verify an existing artifact without downloading, installing, or executing it.
fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: verify_artifact <manifest.json> <platform> <artifact>".into());
    }
    let manifest: rotor_updater::Manifest =
        serde_json::from_slice(&std::fs::read(&args[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let artifact = manifest
        .platforms
        .get(&args[1])
        .ok_or("platform absent from manifest")?;
    rotor_updater::verify_file(
        std::path::Path::new(&args[2]),
        &artifact.signature,
        rotor_updater::PUBLIC_KEY,
    )?;
    println!("Verified Rotor {} {}", manifest.version, args[1]);
    Ok(())
}
