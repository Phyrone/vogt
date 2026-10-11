import Database from '@tauri-apps/plugin-sql';
import { drizzle } from 'drizzle-orm/sqlite-proxy';
import * as schema from './schema.ts';

let sqlite: Database | undefined;

/**
 * Open the app's database. Must have resolved before the first query.
 *
 * The name is the one `src-tauri/src/lib.rs` registers the migrations under, which the
 * Rust side applies while loading.
 */
export async function initDatabase() {
	sqlite = await Database.load('sqlite:vogt.db');
}

/** Queries run in SQLite on the Rust side, through tauri-plugin-sql. */
export const db = drizzle(
	async (sql, params, method) => {
		if (!sqlite) throw new Error('The database is not open');
		if (method === 'run') {
			await sqlite.execute(sql, params);
			return { rows: [] };
		}
		// shortcut: rows arrive keyed by column name, so two selected columns with the same name
		// collapse into one; alias them when a join needs both
		const rows = (await sqlite.select<Record<string, unknown>[]>(sql, params)).map((row) =>
			Object.values(row)
		);
		return { rows: method === 'get' ? (rows[0] as unknown[]) : rows };
	},
	{ schema }
);
