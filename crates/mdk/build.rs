// Embed the app icon in mdk.exe. GPUI loads icon resource 1 for the window and taskbar,
// and Explorer and the NSIS shortcuts read the same resource.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/app/icon.ico")
            .compile()
            .expect("embed the Windows icon");
    }
}
