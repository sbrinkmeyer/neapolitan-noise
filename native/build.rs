fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "Neapolitan Noise - white, brown and pink noise generator");
        res.set("ProductName", "Neapolitan Noise");
        if let Err(err) = res.compile() {
            // Don't fail the build over a missing icon toolchain; ship the exe.
            println!("cargo:warning=could not embed Windows resources: {err}");
        }
    }
}
