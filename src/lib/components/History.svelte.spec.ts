import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
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
