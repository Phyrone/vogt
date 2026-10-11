import { expect, test } from 'vitest';
import type { Opened as GitSummary } from '#lib/bindings/Opened.ts';
import type { RepoRow } from '#lib/db/repo.ts';
import { ago, badges, layoutWindow, toHistoryEdge, toHistoryRow, toRepository } from './history.ts';
import type { Commit } from './types.ts';

test('a time is told the way the history column has room for', () => {
	const now = Date.UTC(2026, 9, 11, 12) / 1000;
	const minutes = (n: number) => now - n * 60;
	expect([0.5, 12, 59, 60, 23 * 60].map((n) => ago(minutes(n), now))).toEqual([
		'now',
		'12 min',
		'59 min',
		'1 h',
		'23 h'
	]);
	expect([1, 2, 29, 30].map((days) => ago(minutes(days * 24 * 60), now))).toEqual([
		'yesterday',
		'2 d',
		'29 d',
		'2026-09-11'
	]);
});

test('each line gets a letter of its own while its name has one left', () => {
	expect(badges(['main', 'feat/commit-graph', 'feat/composer', 'fix/cog', 'co', '#'])).toEqual([
		'M',
		'C',
		'O',
		'G',
		'C',
		'?'
	]);
});

test('a repository read from git gets the badges and colours of its lines', () => {
	const summary: GitSummary = {
		user: 'Mira',
		trunk: 'main',
		headLine: 'main',
		head: 'b',
		ahead: 2,
		lines: [
			{ id: 'main', name: 'main', remote: true, merged: false },
			{ id: 'merged-a', name: 'feat/x', remote: false, merged: true }
		],
		stashes: [],
		tags: [],
		cachePath: '/tmp/repo.db',
		view: { id: 'all', only: [], total: 0, lanes: 1, version: 0 },
		progress: { indexed: 0, laid: 0, complete: true }
	};

	const repository = toRepository(summary);

	expect(repository.lines['merged-a']).toMatchObject({ badge: 'X', color: 'secondary' });
	expect(repository.lines.main).toMatchObject({ badge: 'M', color: 'primary', remote: true });
	expect(repository).toMatchObject({ ahead: 2, changes: [] });
	expect(repository.commits).toBeUndefined();
});

test('rows read from the cache get their relative times and parsed facts', () => {
	const row: RepoRow = {
		row: 4,
		lane: 1,
		hash: 'b',
		subject: 's',
		author: 'Mira',
		time: 940,
		body: null,
		parents: '["a"]',
		line: 'main',
		refs: '[{"kind":"tag","label":"v1"}]',
		pushed: 1
	};

	const mapped = toHistoryRow(row, 1_000_000);

	expect(mapped).toMatchObject({
		when: '1 min',
		body: undefined,
		parents: ['a'],
		refs: [{ kind: 'tag', label: 'v1' }],
		pushed: true,
		row: 4,
		lane: 1
	});
});

test('edges read from the cache get their points parsed', () => {
	expect(toHistoryEdge({ from: 0, to: 3, line: 'feat', points: '[[1,0],[1,2],[0,3]]' })).toEqual({
		from: 0,
		to: 3,
		line: 'feat',
		points: [
			[1, 0],
			[1, 2],
			[0, 3]
		]
	});
});

test('commits held in the webview are cut into the same windows', () => {
	const commit = (hash: string, line: string, parents: string[]): Commit => ({
		hash,
		line,
		parents,
		subject: hash,
		author: '',
		when: '',
		pushed: true
	});
	// a branch leaves the root and is merged at the top, passing three rows of trunk
	const commits = [
		commit('merge', 'main', ['m3', 'b1']),
		commit('m3', 'main', ['m2']),
		commit('m2', 'main', ['m1']),
		commit('b1', 'feat', ['root']),
		commit('m1', 'main', ['root']),
		commit('root', 'main', [])
	];

	const window = layoutWindow(commits, 'main', 2, 3);

	expect(window).toMatchObject({ total: 6, lanes: 2 });
	expect(window.rows).toMatchObject([{ hash: 'm2', row: 2, lane: 0 }]);
	// the edge into the row, the one out of it, and the merged line passing by
	expect(window.edges.map((edge) => `${edge.from}>${edge.to}`).sort()).toEqual([
		'0>3',
		'1>2',
		'2>4'
	]);
});
