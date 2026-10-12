import { beforeEach, expect, test, vi } from 'vitest';
import type { Changed } from '#lib/bindings/Changed.ts';
import { readRepository, type Opened } from './history.ts';
import { RepoSession } from './session.svelte.ts';

// the backend's events, handed to whoever listens
const backend = vi.hoisted(() => ({ emit: (() => {}) as (changed: Changed) => void }));
const emit = (changed: Changed) => backend.emit(changed);

vi.mock('@tauri-apps/api/core', async (original) => ({
	...(await original<typeof import('@tauri-apps/api/core')>()),
	isTauri: () => true
}));
vi.mock('@tauri-apps/api/event', () => ({
	listen: async (_event: string, handler: (event: { payload: Changed }) => void) => {
		backend.emit = (payload) => handler({ payload });
		return () => {};
	}
}));
vi.mock('./history.ts', async (original) => ({
	...(await original<typeof import('./history.ts')>()),
	readRepository: vi.fn()
}));

// a real repository whose history is not laid out yet; the cache is never read here
function opened(version: number): Opened {
	return {
		repository: {
			user: 'Vogt',
			trunk: 'main',
			headLine: 'main',
			head: 'abc1234',
			lines: {},
			changes: [],
			stashes: [],
			tags: []
		},
		db: null,
		view: { id: 'all', only: [], total: 0, lanes: 1, version },
		progress: { indexed: 0, laid: 0, complete: true }
	};
}

const changed = (version: number): Changed => ({
	repo: 'repo',
	version,
	indexed: 0,
	laid: 0,
	complete: true
});

const settle = () => new Promise((done) => setTimeout(done, 0));

beforeEach(() => vi.mocked(readRepository).mockReset());

test('a refresh that failed is asked for again by the next event', async () => {
	const error = vi.spyOn(console, 'error').mockImplementation(() => {});
	const session = new RepoSession(opened(1), 'ws', 'repo', '/tmp/repo');
	await session.listen();

	vi.mocked(readRepository).mockRejectedValueOnce(new Error('database is locked'));
	emit(changed(2));
	await settle();
	expect(error).toHaveBeenCalledWith(
		'vogt: the history could not be read again',
		expect.any(Error)
	);
	expect(session.version).toBe(1);

	vi.mocked(readRepository).mockResolvedValueOnce(opened(2));
	emit(changed(2));
	await settle();
	expect(readRepository).toHaveBeenCalledTimes(2);
	expect(session.version).toBe(2);
	error.mockRestore();
});
