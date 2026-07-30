fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "Brownie brown-noise generator");
        res.set("ProductName", "Brownie");
        if let Err(err) = res.compile() {
            // Don't fail the build over a missing icon toolchain; ship the exe.
            println!("cargo:warning=could not embed Windows resources: {err}");
        }
    }
}
