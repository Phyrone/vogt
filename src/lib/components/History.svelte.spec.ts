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
