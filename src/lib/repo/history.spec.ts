import { expect, test } from 'vitest';
import { ago, badges, toRepository, type GitHistory } from './history.ts';

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

test('a history read from git becomes a repository the interface can draw', () => {
	const history: GitHistory = {
		user: 'Mira',
		trunk: 'main',
		headLine: 'main',
		head: 'b',
		lines: [
			{ id: 'main', name: 'main', remote: true, merged: false },
			{ id: 'merged-a', name: 'feat/x', remote: false, merged: true }
		],
		commits: [
			{
				hash: 'b',
				subject: 's',
				body: null,
				author: 'Mira',
				time: 940,
				line: 'main',
				parents: ['a'],
				refs: [],
				pushed: true
			}
		],
		stashes: [],
		tags: []
	};

	const repository = toRepository(history, 1_000_000);

	expect(repository.lines['merged-a']).toMatchObject({ badge: 'X', color: 'secondary' });
	expect(repository.lines.main).toMatchObject({ badge: 'M', color: 'primary', remote: true });
	expect(repository.commits[0]).toMatchObject({ when: '1 min', body: undefined });
	expect(repository.commits[0]).not.toHaveProperty('files');
});
