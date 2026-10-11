fn main() {
    // sql_migrations! reads these directories; a new migration has to trigger a rebuild
    println!("cargo:rerun-if-changed=../drizzle");
    println!("cargo:rerun-if-changed=../drizzle-repo");
    tauri_build::build()
}
