import { createRawSnippet } from 'svelte';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { RepoSession } from '#lib/repo/session.svelte.ts';
import type { Repository } from '#lib/repo/types.ts';
import Layout from './+layout.svelte';

// the smallest repository the session can carry; no lines, no commits, nothing to draw
const repository: Repository = {
	user: 'Vogt',
	trunk: 'main',
	headLine: 'main',
	head: 'abc1234',
	lines: {},
	changes: [],
	stashes: [],
	tags: []
};

test('a listener that resolves after the layout is gone is stopped at once', async () => {
	let resolve!: (off: () => void) => void;
	const off = vi.fn();
	const session = new RepoSession(
		{ repository, db: null, view: null, progress: { indexed: 0, laid: 0, complete: true } },
		'ws',
		'repo',
		'/tmp/repo'
	);
	session.listen = vi.fn(
		() =>
			new Promise<() => void>((settle) => {
				resolve = settle;
			})
	);
	const data = {
		workspaces: [{ id: 'ws', name: 'Work', repos: [] }],
		workspace: { id: 'ws', name: 'Work', repos: [] },
		repo: { id: 'repo', name: 'repo', path: '/tmp/repo' },
		worktrees: [],
		worktree: undefined,
		session
	};
	const children = createRawSnippet(() => ({ render: () => '<span></span>' }));

	const { unmount } = await render(Layout, { data, children });
	// the layout is mounted and its listener is waiting for the backend
	expect(session.listen).toHaveBeenCalledOnce();
	await unmount();
	// the backend answers only now, after the layout is gone
	resolve(off);
	await new Promise((done) => setTimeout(done, 0));
	expect(off).toHaveBeenCalledOnce();
});
