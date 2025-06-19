fn main() {
    #[cfg(target_os = "windows")] // TODO: enable for macOS
    embed_resource::compile("./assets/application.rc", embed_resource::NONE);
    let config = 
        slint_build::CompilerConfiguration::new()
        .with_bundled_translations("./assets/lang")
        .with_style("fluent".into());
    slint_build::compile_with_config("src/ui/windows.slint", config).expect("slint_build compile failed");
}