use std::{env, path::Path};

fn main() {
    // sql_migrations! reads these directories; a new migration has to trigger a rebuild
    println!("cargo:rerun-if-changed=../drizzle");
    println!("cargo:rerun-if-changed=../drizzle-repo");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return tauri_build::build();
    }

    // tauri-build puts the application manifest into the app binary only. The
    // manifest asks for Common Controls v6, which the dialog plugin imports
    // from, so a test binary without it is not even loaded: "the specified
    // procedure could not be found". The linker embeds the manifest in every
    // binary instead, and tauri-build leaves its own copy out so there is one.
    let manifest =
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());

    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
