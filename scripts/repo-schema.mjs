// Builds a database with the repository cache schema from `drizzle-repo/`, for `cargo sqlx
// prepare` to check the Rust queries against. Run through `pnpm db:repo:prepare`.
import { mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';

const out = 'src-tauri/target/repo-schema.db';
rmSync(out, { force: true });
mkdirSync('src-tauri/target', { recursive: true });
const sqlite = new DatabaseSync(out);
for (const file of readdirSync('drizzle-repo')
	.filter((file) => file.endsWith('.sql'))
	.sort()) {
	sqlite.exec(readFileSync(`drizzle-repo/${file}`, 'utf8'));
}
// The table the applied migrations are recorded in. `cache::migrate` creates it by hand, not
// through a migration, so it is added here for the queries on it to be checked; this must
// match `MIGRATION_TABLE` in `src-tauri/src/cache.rs`.
sqlite.exec(`CREATE TABLE IF NOT EXISTS migration (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL
)`);
sqlite.close();
console.log(`Repository cache schema built at ${out}`);
