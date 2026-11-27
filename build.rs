fn main() {
    println!("cargo:rerun-if-changed=assets/windows/copywheel.ico");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/windows/copywheel.ico");
        res.set("ProductName", "CopyWheel");
        res.set("FileDescription", "CopyWheel");
        res.set("InternalName", "CopyWheel");
        res.set("OriginalFilename", "copywheel.exe");
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(err) = res.compile() {
            panic!("failed to compile Windows resources: {err}");
        }
    }
}
