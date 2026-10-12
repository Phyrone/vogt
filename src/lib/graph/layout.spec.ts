import { describe, expect, it } from 'vitest';
import { repository } from '#lib/repo/mock.ts';
import type { Commit } from '#lib/repo/types.ts';
import golden from './layout.golden.json';
import { graphWidth, INSET, layoutGraph, PITCH } from './layout';

const commit = (hash: string, line: string, parents: string[]): Commit => ({
	hash,
	line,
	parents,
	subject: hash,
	author: '',
	when: '',
	pushed: true,
	files: []
});

describe('layoutGraph', () => {
	it('keeps the trunk on lane 0 and gives a branch the next lane', () => {
		const layout = layoutGraph(
			[commit('b2', 'feat', ['b1']), commit('b1', 'feat', ['m1']), commit('m1', 'main', [])],
			'main'
		);
		expect(layout.lanes).toBe(2);
		expect(layout.nodes.map((n) => n.lane)).toEqual([1, 1, 0]);
	});

	it('bends a branch into its parent line at 45°', () => {
		const layout = layoutGraph(
			[
				commit('b1', 'feat', ['m1']),
				commit('m3', 'main', ['m2']),
				commit('m2', 'main', ['m1']),
				commit('m1', 'main', [])
			],
			'main'
		);
		const edge = layout.edges.find((e) => e.from === 'b1');
		expect(edge?.line).toBe('feat');
		// runs down its own lane, then crosses one lane over one row
		expect(edge?.points).toEqual([
			[1, 0],
			[1, 2],
			[0, 3]
		]);
	});

	it('marks merges as interchanges and colours the edge by the merged line', () => {
		const layout = layoutGraph(
			[commit('m2', 'main', ['m1', 's1']), commit('s1', 'side', ['m1']), commit('m1', 'main', [])],
			'main'
		);
		expect(layout.nodes[0].kind).toBe('interchange');
		expect(layout.edges.find((e) => e.to === 's1')?.line).toBe('side');
	});

	it('reuses a lane once the line that held it has rejoined', () => {
		const layout = layoutGraph(repository.commits!, repository.trunk);
		const laneOf = (hash: string) => layout.nodes.find((n) => n.hash === hash)?.lane;
		expect(layout.lanes).toBe(4);
		expect(laneOf('a41f9c2')).toBe(1);
		expect(laneOf('8b44e21')).toBe(1);
		expect(laneOf('e5c8a70')).toBe(2);
		expect(laneOf('19b6f4d')).toBe(3);
	});

	// the same fixture is laid out by `src-tauri/src/layout.rs`, so the two layouts cannot drift apart
	it('lays the golden history out as the Rust layout does', () => {
		const commits = golden.commits.map((c) => commit(c.hash, c.line, c.parents));
		const layout = layoutGraph(commits, golden.trunk);
		const row = new Map(commits.map((c, index) => [c.hash, index]));
		expect(layout.lanes).toBe(golden.lanes);
		expect(layout.nodes.map((n) => n.lane)).toEqual(golden.lane);
		expect(
			layout.edges
				.map(({ from, to, line, points }) => ({
					from: row.get(from),
					to: row.get(to),
					line,
					points
				}))
				.sort((a, b) => a.from! - b.from! || a.to! - b.to!)
		).toEqual(golden.edges);
	});

	it('draws uncommitted work as a pending station', () => {
		const pending = { ...commit('wip', 'feat', ['b1']), pending: true };
		const layout = layoutGraph([pending, commit('b1', 'feat', [])], 'feat');
		expect(layout.nodes[0].kind).toBe('pending');
		expect(layout.edges[0].pending).toBe(true);
	});
});

describe('graphWidth', () => {
	it('leaves an inset on both sides of the lanes', () => {
		expect(graphWidth(1)).toBe(INSET * 2);
		expect(graphWidth(4)).toBe(INSET * 2 + 3 * PITCH);
	});
});
