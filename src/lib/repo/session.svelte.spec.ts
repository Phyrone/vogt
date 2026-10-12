import { beforeEach, expect, onTestFinished, test, vi } from 'vitest';
import type { Changed } from '#lib/bindings/Changed.ts';
import type { RepoDatabase } from '#lib/db/repo.ts';
import { materializeView, readRepository, type Opened } from './history.ts';
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
	readRepository: vi.fn(),
	materializeView: vi.fn()
}));
vi.mock('#lib/db/repo.ts', async (original) => ({
	...(await original<typeof import('#lib/db/repo.ts')>()),
	window: async () => ({ rows: [], edges: [] }),
	locate: async () => null
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

beforeEach(() => {
	vi.mocked(readRepository).mockReset();
	vi.mocked(materializeView).mockReset();
});

test('a refresh that failed is tried once more, then left to the next event', async () => {
	vi.useFakeTimers();
	const error = vi.spyOn(console, 'error').mockImplementation(() => {});
	onTestFinished(() => {
		vi.useRealTimers();
		error.mockRestore();
	});
	const session = new RepoSession(opened(1), 'ws', 'repo', '/tmp/repo');
	await session.listen();

	// the round's one last event finds the cache busy, and so does the try after it
	vi.mocked(readRepository).mockRejectedValue(new Error('database is locked'));
	emit(changed(2));
	await vi.advanceTimersByTimeAsync(0);
	expect(error).toHaveBeenCalledWith(
		'vogt: the history could not be read again',
		expect.any(Error)
	);
	expect(readRepository).toHaveBeenCalledOnce();
	await vi.advanceTimersByTimeAsync(1000);
	expect(readRepository).toHaveBeenCalledTimes(2);
	// one try more, not one a second for as long as the cache is busy
	await vi.advanceTimersByTimeAsync(10_000);
	expect(readRepository).toHaveBeenCalledTimes(2);
	expect(session.version).toBe(1);

	vi.mocked(readRepository).mockResolvedValue(opened(2));
	emit(changed(2));
	await vi.advanceTimersByTimeAsync(0);
	expect(readRepository).toHaveBeenCalledTimes(3);
	expect(session.version).toBe(2);
});

test('a refresh that failed is not tried again once nobody listens', async () => {
	vi.useFakeTimers();
	const error = vi.spyOn(console, 'error').mockImplementation(() => {});
	onTestFinished(() => {
		vi.useRealTimers();
		error.mockRestore();
	});
	const session = new RepoSession(opened(1), 'ws', 'repo', '/tmp/repo');
	const stop = await session.listen();

	vi.mocked(readRepository).mockRejectedValue(new Error('database is locked'));
	emit(changed(2));
	await vi.advanceTimersByTimeAsync(0);
	stop();
	await vi.advanceTimersByTimeAsync(1000);
	expect(readRepository).toHaveBeenCalledOnce();
});

test('one mask is laid out once for all the chunks that ask for it', async () => {
	const session = new RepoSession(
		{ ...opened(1), db: {} as RepoDatabase },
		'ws',
		'repo',
		'/tmp/repo'
	);
	const masked = { id: 'v', only: ['a', 'b'], total: 0, lanes: 1, version: 1 };
	vi.mocked(materializeView).mockResolvedValue(masked);

	// the first paint asks for two chunks at once, the lines in any order
	await Promise.all([session.rows(0, 100, ['b', 'a']), session.rows(100, 200, ['a', 'b', 'a'])]);
	expect(materializeView).toHaveBeenCalledOnce();

	// a new version lays it out again
	vi.mocked(readRepository).mockResolvedValueOnce({ ...opened(2), db: {} as RepoDatabase });
	await session.refresh();
	await session.rows(0, 100, ['a', 'b']);
	expect(materializeView).toHaveBeenCalledTimes(2);

	// and a mask that failed is asked for again
	vi.mocked(materializeView).mockRejectedValueOnce(new Error('busy'));
	await expect(session.rows(0, 100, ['c'])).rejects.toThrow('busy');
	await session.rows(0, 100, ['c']);
	expect(materializeView).toHaveBeenCalledTimes(4);
});

test('a masked view is laid out again as soon as the history changed', async () => {
	const session = new RepoSession(
		{ ...opened(1), db: {} as RepoDatabase },
		'ws',
		'repo',
		'/tmp/repo'
	);
	await session.listen();
	vi.mocked(materializeView).mockResolvedValue({
		id: 'v',
		only: ['a'],
		total: 0,
		lanes: 1,
		version: 1
	});
	await session.rows(0, 100, ['a']);
	expect(materializeView).toHaveBeenCalledOnce();

	// the backend dropped the masked views with the new layout; the summary is still on its way
	vi.mocked(readRepository).mockReturnValue(new Promise(() => {}));
	emit(changed(2));
	await session.rows(0, 100, ['a']);
	expect(materializeView).toHaveBeenCalledTimes(2);
	expect(session.version).toBe(1);
});
