fn main() {
    embed_resource::compile("./assets/application.rc", embed_resource::NONE);
    slint_build::compile("src/ui/windows.slint").expect("slint_build compile failed");
}