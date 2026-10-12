import { expect, onTestFinished, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import { PITCH } from '#lib/graph/layout.ts';
import type { HistoryRow, HistoryWindow, Line } from '#lib/repo/types.ts';
import History from './History.svelte';

function row(hash: string, line: string, position: number): HistoryRow {
	return {
		hash,
		subject: `commit ${hash}`,
		author: 'Vogt',
		when: 'now',
		line,
		parents: [],
		pushed: true,
		row: position,
		lane: 0
	};
}

test('a row whose line has not arrived yet renders without an error', async () => {
	const rows = [row('a', 'fresh-branch', 0), row('b', 'main', 1)];
	const source = {
		rows: vi.fn(async (from: number, to: number): Promise<HistoryWindow> => ({
			total: rows.length,
			lanes: 2,
			rows: rows.slice(from, to),
			edges: []
		})),
		locate: vi.fn(async () => null),
		version: 1,
		progress: { indexed: 2, laid: 2, complete: true }
	};
	const lines: Record<string, Line> = {
		main: { id: 'main', name: 'main', badge: 'M', color: 'primary', remote: false }
	};

	// a sized container, so the virtual list has a viewport to measure
	const container = document.createElement('div');
	container.style.cssText = 'display: flex; height: 480px;';
	document.body.append(container);
	await render(History, {
		props: { source, lines, trunk: 'main', head: 'b', selected: null, picked: [] },
		target: container
	});

	await expect.element(page.getByText('commit a')).toBeInTheDocument();
	await expect.element(page.getByText('commit b')).toBeInTheDocument();
	// the missing line shows its fallback name instead of crashing
	await expect.element(page.getByText('on fresh-branch')).toBeInTheDocument();
});

const main: Record<string, Line> = {
	main: { id: 'main', name: 'main', badge: 'M', color: 'primary', remote: false }
};

/** Renders the history in a list that scrolls by itself; the app's stylesheet is not loaded here. */
async function mountScrolling(props: ComponentProps<typeof History>) {
	const style = document.createElement('style');
	style.textContent = `
		section[aria-label='History'] { display: flex; flex-direction: column; height: 480px; }
		section[aria-label='History'] > .overflow-y-auto { flex: 1; min-height: 0; overflow-y: auto; }
	`;
	document.head.append(style);
	const container = document.createElement('div');
	document.body.append(container);
	// taken out again even when an assertion failed, so the next test starts on a clean page
	onTestFinished(() => {
		container.remove();
		style.remove();
	});
	const rendered = await render(History, { props, target: container });
	return { ...rendered, scroller: container.querySelector<HTMLElement>('.overflow-y-auto')! };
}

const subject = (hash: string) => page.getByText(`commit ${hash}`, { exact: true });

test('chunks far out of view are let go and read again when they come back', async () => {
	const total = 2000;
	const source = {
		rows: vi.fn(async (from: number, to: number): Promise<HistoryWindow> => ({
			total,
			lanes: 1,
			rows: Array.from({ length: Math.min(to, total) - from }, (_, index) =>
				row(`c${from + index}`, 'main', from + index)
			),
			edges: []
		})),
		locate: vi.fn(async () => null),
		version: 1,
		progress: { indexed: total, laid: total, complete: true }
	};
	const { scroller } = await mountScrolling({
		source,
		lines: main,
		trunk: 'main',
		head: 'c0',
		selected: null,
		picked: []
	});
	const asked = (from: number) => source.rows.mock.calls.filter(([start]) => start === from);
	await expect.element(subject('c0')).toBeInTheDocument();
	expect(asked(0)).toHaveLength(1);

	scroller.scrollTop = scroller.scrollHeight;
	await expect.element(subject('c1999')).toBeInTheDocument();
	// a chunk close to the view is held: a step up and back reads nothing twice
	scroller.scrollTop = 1850 * PITCH;
	await expect.element(subject('c1850')).toBeInTheDocument();
	scroller.scrollTop = scroller.scrollHeight;
	await expect.element(subject('c1999')).toBeInTheDocument();
	expect(asked(1900)).toHaveLength(1);

	// back at the top, the first rows are asked for again
	scroller.scrollTop = 0;
	await expect.element(subject('c0')).toBeInTheDocument();
	expect(asked(0)).toHaveLength(2);
});

test('a tag and a remote branch of the same name on one commit both show', async () => {
	const rows: HistoryRow[] = [
		{
			...row('a', 'main', 0),
			refs: [
				{ kind: 'remote', label: 'origin/x' },
				{ kind: 'tag', label: 'origin/x' }
			]
		}
	];
	const source = {
		rows: vi.fn(async (): Promise<HistoryWindow> => ({ total: 1, lanes: 1, rows, edges: [] })),
		locate: vi.fn(async () => null),
		version: 1,
		progress: { indexed: 1, laid: 1, complete: true }
	};
	const lines: Record<string, Line> = {
		main: { id: 'main', name: 'main', badge: 'M', color: 'primary', remote: false }
	};
	const container = document.createElement('div');
	container.style.cssText = 'display: flex; height: 480px;';
	document.body.append(container);
	await render(History, {
		props: { source, lines, trunk: 'main', head: 'a', selected: null, picked: [] },
		target: container
	});

	await expect.element(page.getByText('commit a')).toBeInTheDocument();
	expect(page.getByText('origin/x', { exact: true }).elements()).toHaveLength(2);
});

test('a refresh far down the history puts the reader back on the same commit', async () => {
	// ten commits arrive on top: every row moves down by ten
	const history = (version: number, total: number) => {
		const added = version > 1 ? 10 : 0;
		const hash = (position: number) => (position < added ? `n${position}` : `c${position - added}`);
		return {
			rows: vi.fn(async (from: number, to: number): Promise<HistoryWindow> => ({
				total: total + added,
				lanes: 1,
				rows: Array.from({ length: Math.min(to, total + added) - from }, (_, index) =>
					row(hash(from + index), 'main', from + index)
				),
				edges: []
			})),
			locate: vi.fn(async (hash: string) => Number(hash.slice(1)) + added),
			version,
			progress: { indexed: total, laid: total, complete: true }
		};
	};
	const props = { lines: main, trunk: 'main', head: 'c0', selected: null, picked: [] };
	const { scroller, rerender } = await mountScrolling({ source: history(1, 2000), ...props });
	await expect.element(subject('c0')).toBeInTheDocument();
	scroller.scrollTop = 1500 * PITCH;
	await expect.element(subject('c1500')).toBeInTheDocument();
	const before = scroller.scrollTop;

	const source = history(2, 2000);
	await rerender({ source });
	await expect.poll(() => source.locate.mock.calls).toHaveLength(1);
	const [[anchor]] = source.locate.mock.calls;
	// the commit that led the rows in view leads them again, ten rows further down
	await expect.poll(() => scroller.scrollTop).toBe(before + 10 * PITCH);
	await expect.element(subject(anchor)).toBeInTheDocument();
});

test('a refresh at the top of the history shows the new commit at the top', async () => {
	const history = (version: number) => {
		const added = version > 1 ? 1 : 0;
		const hash = (position: number) => (position < added ? 'n0' : `c${position - added}`);
		return {
			rows: vi.fn(async (from: number, to: number): Promise<HistoryWindow> => ({
				total: 50 + added,
				lanes: 1,
				rows: Array.from({ length: Math.min(to, 50 + added) - from }, (_, index) =>
					row(hash(from + index), 'main', from + index)
				),
				edges: []
			})),
			locate: vi.fn(async (hash: string) => (hash === 'n0' ? 0 : Number(hash.slice(1)) + added)),
			version,
			progress: { indexed: 50, laid: 50, complete: true }
		};
	};
	const props = { lines: main, trunk: 'main', head: 'c0', selected: null, picked: [] };
	const { scroller, rerender } = await mountScrolling({ source: history(1), ...props });
	await expect.element(subject('c0')).toBeInTheDocument();

	const source = history(2);
	await rerender({ source });
	await expect.element(subject('n0')).toBeInTheDocument();
	await new Promise((done) => requestAnimationFrame(done));
	// nothing is kept in view: the list stays at the top, where the new commit is
	expect(source.locate).not.toHaveBeenCalled();
	expect(scroller.scrollTop).toBe(0);
});

test('an answer for a chunk let go while it was on its way is not kept', async () => {
	const total = 2000;
	const answer = (from: number, to: number, name: string): HistoryWindow => ({
		total,
		lanes: 1,
		rows: Array.from({ length: to - from }, (_, index) =>
			row(`${name}${from + index}`, 'main', from + index)
		),
		edges: []
	});
	// the first rows answer at once, the others when the test says so
	const waiting = new Map<number, PromiseWithResolvers<HistoryWindow>>();
	const source = {
		rows: vi.fn(async (from: number, to: number): Promise<HistoryWindow> => {
			if (from === 0) return answer(from, to, 'c');
			const asked = Promise.withResolvers<HistoryWindow>();
			waiting.set(from, asked);
			return asked.promise;
		}),
		locate: vi.fn(async () => null),
		version: 1,
		progress: { indexed: total, laid: total, complete: true }
	};
	const { scroller } = await mountScrolling({
		source,
		lines: main,
		trunk: 'main',
		head: 'c0',
		selected: null,
		picked: []
	});
	const settled = () => new Promise((done) => requestAnimationFrame(done));
	await expect.element(subject('c0')).toBeInTheDocument();

	// the reader passes rows 900 to 1099 and is back at the top before they arrive
	scroller.scrollTop = 1000 * PITCH;
	await expect.poll(() => [...waiting.keys()]).toEqual([900, 1000]);
	scroller.scrollTop = 0;
	await expect.element(subject('c0')).toBeInTheDocument();
	waiting.get(900)!.reject(new Error('the cache was closed'));
	waiting.get(1000)!.resolve(answer(1000, 1100, 'c'));
	await settled();
	// the failure of a chunk nobody looks at any more is not shown
	expect(page.getByRole('alert').elements()).toHaveLength(0);

	// nor are the rows that did arrive kept: back down they are skeletons until read again
	waiting.clear();
	scroller.scrollTop = 1000 * PITCH;
	await expect.poll(() => [...waiting.keys()]).toEqual([900, 1000]);
	await settled();
	expect(subject('c1000').elements()).toHaveLength(0);
	waiting.get(1000)!.resolve(answer(1000, 1100, 'd'));
	await expect.element(subject('d1000')).toBeInTheDocument();
});
