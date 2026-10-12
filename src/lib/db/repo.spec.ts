import { expect, test, vi } from 'vitest';
import type { ViewInfo } from '#lib/bindings/ViewInfo.ts';
import { locate, loadRepoDatabase, window } from './repo.ts';

// the repositories' cache files, backed by in-memory SQLite with the real migrations
const opened = new Map<string, InstanceType<(typeof import('node:sqlite'))['DatabaseSync']>>();
// paths whose next load fails, as a locked file does
const failing = new Set<string>();

vi.mock('@tauri-apps/plugin-sql', async () => {
	const { readdirSync, readFileSync } = await import('node:fs');
	const { DatabaseSync } = await import('node:sqlite');
	return {
		default: {
			async load(path: string) {
				if (failing.delete(path)) throw new Error('database is locked');
				const sqlite = new DatabaseSync(':memory:');
				for (const file of readdirSync('drizzle-repo')
					.filter((file) => file.endsWith('.sql'))
					.sort()) {
					sqlite.exec(readFileSync(`drizzle-repo/${file}`, 'utf8'));
				}
				opened.set(path, sqlite);
				return {
					select: async (sql: string, params: never[]) => sqlite.prepare(sql).all(...params),
					execute: async (sql: string, params: never[]) => sqlite.prepare(sql).run(...params),
					close: async () => true
				};
			}
		}
	};
});

test('a window and a locate read the cache the Rust side writes', async () => {
	const path = '/tmp/vogt-repo-spec.db';
	const repo = await loadRepoDatabase('repo-1', path);
	const sqlite = opened.get(`sqlite:${path}`)!;
	sqlite.exec(`
		INSERT INTO commits (hash, time, parents, subject, author, body, expanded) VALUES
			('c3', 300, '["c2"]', 'third', 'Mira', NULL, 1),
			('c2', 200, '["c1"]', 'second', 'Mira', 'why it exists', 1),
			('c1', 100, '[]', 'first', 'Mira', NULL, 1);
		INSERT INTO node (view_id, row, hash, lane, line, pushed, refs) VALUES
			('all', 0, 'c3', 0, 'main', 1, '[]'),
			('all', 1, 'c2', 0, 'main', 0, '[{"kind":"tag","label":"v1"}]'),
			('all', 2, 'c1', 1, 'feat', 1, '[]');
		INSERT INTO edge (view_id, "from", "to", line, points) VALUES
			('all', 0, 1, 'main', '[[0,0],[0,1]]'),
			('all', 1, 2, 'feat', '[[0,1],[1,2]]');
		INSERT INTO views (id, only_lines, total, lanes, version) VALUES ('all', '[]', 3, 2, 7);
	`);

	const view: ViewInfo = { id: 'all', only: [], total: 3, lanes: 2, version: 7 };
	const { rows, edges } = await window(repo, view, 0, 10);

	expect(rows.map((row) => [row.row, row.hash, row.lane, row.line])).toEqual([
		[0, 'c3', 0, 'main'],
		[1, 'c2', 0, 'main'],
		[2, 'c1', 1, 'feat']
	]);
	// the joined commit facts arrive with the row
	expect(rows[1]).toMatchObject({
		subject: 'second',
		author: 'Mira',
		body: 'why it exists',
		parents: '["c1"]',
		pushed: 0
	});
	expect(edges).toEqual([
		{ from: 0, to: 1, line: 'main', points: '[[0,0],[0,1]]' },
		{ from: 1, to: 2, line: 'feat', points: '[[0,1],[1,2]]' }
	]);

	// only the rows of the window arrive
	expect((await window(repo, view, 1, 2)).rows.map((row) => row.hash)).toEqual(['c2']);
	// and a commit is found by its hash, not its row
	expect(await locate(repo, 'c1')).toBe(2);
	expect(await locate(repo, 'gone')).toBeNull();
});

test('a cache file that failed to load is tried again', async () => {
	const path = '/tmp/vogt-repo-spec-locked.db';
	failing.add(`sqlite:${path}`);
	await expect(loadRepoDatabase('repo-locked', path)).rejects.toThrow('database is locked');

	const repo = await loadRepoDatabase('repo-locked', path);
	expect(repo.db).toBeDefined();
	// and once open, the same connection is handed out again
	expect(await loadRepoDatabase('repo-locked', path)).toBe(repo);
});
