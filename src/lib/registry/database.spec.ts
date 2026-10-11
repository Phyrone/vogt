import { expect, test, vi } from 'vitest';
import { initDatabase } from '#lib/db/index.ts';
import { database } from './database.ts';

// the Tauri side of the registry: nothing to call but the cache removal
const { invoke } = vi.hoisted(() => ({
	invoke: vi.fn(async () => '/tmp/never-opened.db')
}));

vi.mock('@tauri-apps/api/core', () => ({
	invoke,
	isTauri: () => true
}));

// tauri-plugin-sql, backed by an in-memory SQLite that the real migrations set up
vi.mock('@tauri-apps/plugin-sql', async () => {
	const { readdirSync, readFileSync } = await import('node:fs');
	const { DatabaseSync } = await import('node:sqlite');
	return {
		default: {
			async load() {
				const sqlite = new DatabaseSync(':memory:');
				for (const file of readdirSync('drizzle').sort()) {
					if (file.endsWith('.sql')) sqlite.exec(readFileSync(`drizzle/${file}`, 'utf8'));
				}
				return {
					select: async (sql: string, params: never[]) => sqlite.prepare(sql).all(...params),
					execute: async (sql: string, params: never[]) => sqlite.prepare(sql).run(...params),
					close: async () => true
				};
			}
		}
	};
});

test('the registry keeps workspaces and their repositories', async () => {
	await initDatabase();

	const [initial] = await database.listWorkspaces();
	expect(initial).toMatchObject({ name: 'Default', repos: [] });
	await expect(database.createWorkspace(' ')).rejects.toBe('A workspace needs a name');

	const work = await database.createWorkspace(' Work ');
	const api = await database.addRepo(work.id, '/home/me/code/api');
	const web = await database.addRepo(work.id, 'C:\\code\\web\\');
	expect(await database.addRepo(work.id, '/home/me/code/api')).toEqual(api);
	await expect(database.addRepo('gone', '/tmp')).rejects.toThrow();

	expect(await database.listWorkspaces()).toMatchObject([
		{ id: initial!.id, repos: [] },
		{ name: 'Work', repos: [{ name: 'api' }, { id: web.id, name: 'web' }] }
	]);

	await database.removeRepo(work.id, api.id);
	expect((await database.listWorkspaces())[1]!.repos).toEqual([web]);

	// a failed removal leaves the repository in place, so it can be tried again
	invoke.mockRejectedValueOnce('Could not delete the cache file');
	await expect(database.removeRepo(work.id, web.id)).rejects.toBe(
		'Could not delete the cache file'
	);
	expect((await database.listWorkspaces())[1]!.repos).toEqual([web]);
});
