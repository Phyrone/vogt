import { defineConfig } from 'drizzle-kit';

// A repository's cache lives in its own file under the app cache directory. This schema is the
// source of truth for it: drizzle-kit generates the migrations into `drizzle-repo/`, the Rust side
// applies them when a cache file is created (src-tauri/src/cache.rs) and checks its queries against
// a database built from them (`pnpm db:repo:prepare`).
export default defineConfig({
	schema: './src/lib/db/repo-schema.ts',
	dialect: 'sqlite',
	out: './drizzle-repo',
	verbose: true,
	strict: true
});
