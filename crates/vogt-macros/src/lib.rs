use std::{env, fs, path::PathBuf};

use proc_macro::TokenStream;

/// Expands to a `Vec<tauri_plugin_sql::Migration>` holding every `.sql` file
/// of a directory, in file name order and numbered from 1.
///
/// The directory is a string literal relative to the calling crate's manifest.
/// A file `0003_add_tags.sql` becomes the migration described as `add_tags`.
///
/// Cargo does not rerun a macro when a file appears, so the calling crate's
/// build script has to print `cargo:rerun-if-changed` for the directory.
#[proc_macro]
pub fn sql_migrations(input: TokenStream) -> TokenStream {
    let relative = input.to_string();
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let dir = PathBuf::from(manifest).join(relative.trim_matches('"'));

    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();

    let migrations: String = files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            let description = stem.split_once('_').map_or(&*stem, |(_, name)| name);
            format!(
                "::tauri_plugin_sql::Migration {{
                    version: {},
                    description: {description:?},
                    sql: include_str!({file:?}),
                    kind: ::tauri_plugin_sql::MigrationKind::Up,
                }},",
                index + 1
            )
        })
        .collect();

    format!("vec![{migrations}]")
        .parse()
        .expect("generated code is valid Rust")
}
