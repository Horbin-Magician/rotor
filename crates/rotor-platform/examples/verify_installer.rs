fn main() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        if args.len() != 3 {
            return Err("usage: verify_installer <file> <product> <version>".into());
        }
        rotor_platform::installer::verify(std::path::Path::new(&args[0]), &args[1], &args[2])?;
        println!("Installer identity/version verified without execution");
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    Err("Windows installer metadata requires Windows".into())
}
