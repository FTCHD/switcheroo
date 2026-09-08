//! The web bundle is embedded at compile time from `web/dist`. When it is missing (a fresh
//! checkout before `npm run build`) a placeholder page is generated so the Rust side still
//! builds and tests; the real UI needs the Vite build. On Windows targets the app icon and
//! version info are embedded as executable resources.

use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=assets/switcheroo.ico");
    embed_windows_resources();
    let dist = Path::new("web/dist");
    if !dist.join("index.html").exists() {
        fs::create_dir_all(dist).expect("create web/dist");
        fs::write(
            dist.join("index.html"),
            "<!doctype html><meta charset=utf-8><title>Switcheroo</title>\
             <!-- switcheroo:boot --><body style=\"font-family:system-ui;padding:2rem\">\
             <h1>Switcheroo</h1><p>The web UI was not built. Run <code>npm run build</code> in \
             <code>web/</code> and rebuild.</p></body>",
        )
        .expect("write placeholder");
        println!("cargo:warning=web/dist missing: embedded a placeholder page (run `npm run build` in web/)");
    }
}

/// Icon + version info for the `.exe`. Needs `rc.exe` (MSVC) or `llvm-rc`/`windres` when
/// cross-compiling; a missing tool only costs the icon, never the build.
fn embed_windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/switcheroo.ico");
    res.set("ProductName", "Switcheroo");
    res.set("FileDescription", "Switch the signed-in account of developer CLIs");
    res.set("LegalCopyright", "MIT License");
    if let Err(e) = res.compile() {
        println!("cargo:warning=Windows resources not embedded (no resource compiler?): {e}");
    }
}
