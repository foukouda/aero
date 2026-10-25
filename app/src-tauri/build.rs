//! Build script: generates the Tauri context (configuration, icons,
//! permissions) and embeds the Windows application manifest.

use std::path::PathBuf;

fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // tauri-build embeds the manifest (Common Controls v6, needed by the
        // native dialogs) in the application binary only; test executables
        // then fail to start with STATUS_ENTRYPOINT_NOT_FOUND. Embed it in
        // every linked artifact instead.
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let manifest = PathBuf::from(dir).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    }
    if let Err(error) =
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
    {
        eprintln!("tauri-build failed: {error:#}");
        std::process::exit(1);
    }
}
