fn main() {
    // sql_migrations! reads this directory; a new migration has to trigger a rebuild
    println!("cargo:rerun-if-changed=../drizzle");
    tauri_build::build()
}
