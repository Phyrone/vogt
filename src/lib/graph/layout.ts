import type { Commit } from '#lib/repo/types.ts';

/** Row height and lane spacing in pixels. They are equal so that every bend is 45°. */
export const PITCH = 36;
/** Distance from the graph's left edge to the first lane. */
export const INSET = 20;

/**
 * The most lanes the graph column makes room for.
 *
 * shortcut: lanes past this are cut off, which a history with hundreds of parallel lines needs
 * until the layout stops giving every merged branch a lane down to where it forked.
 */
export const MAX_LANES = 12;

export function graphWidth(lanes: number): number {
	return INSET * 2 + Math.max(0, lanes - 1) * PITCH;
}

export interface GraphNode {
	hash: string;
	line: string;
	lane: number;
	row: number;
	kind: 'station' | 'interchange' | 'pending';
}

export interface GraphEdge {
	from: string;
	to: string;
	/** The line whose colour the edge takes: the one that leaves or joins the other. */
	line: string;
	/** Corner points as [lane, row]. */
	points: [number, number][];
	pending: boolean;
}

export interface GraphLayout {
	lanes: number;
	nodes: GraphNode[];
	edges: GraphEdge[];
}

/**
 * Lay a history out as a line map. `commits` are newest first, one per row.
 *
 * Only the synthetic history is laid out here. A real one is laid out by the same algorithm in
 * `src-tauri/src/layout.rs`; a change to one belongs in the other.
 *
 * Each line keeps one lane for as long as it runs: `trunk` takes lane 0, every other line the
 * innermost lane that is free between where it leaves its parent line and where it rejoins.
 */
export function layoutGraph(commits: Commit[], trunk: string): GraphLayout {
	const row = new Map(commits.map((c, i) => [c.hash, i]));
	const lineOf = new Map(commits.map((c) => [c.hash, c.line]));

	// rows each line needs its lane for
	const spans = new Map<string, [number, number]>();
	const widen = (line: string, from: number, to: number) => {
		const s = spans.get(line);
		spans.set(line, s ? [Math.min(s[0], from), Math.max(s[1], to)] : [from, to]);
	};
	commits.forEach((c, i) => {
		widen(c.line, i, i);
		for (const p of c.parents) {
			const j = row.get(p);
			const parentLine = lineOf.get(p);
			if (j === undefined || parentLine === undefined || parentLine === c.line) continue;
			// the outer line is the one that turns, so it holds its lane up to the bend
			if (parentLine === trunk || c.parents[0] === p) widen(c.line, i, j - 1);
			else widen(parentLine, i + 1, j);
		}
	});

	const lane = new Map<string, number>([[trunk, 0]]);
	const taken: [number, number][][] = [[]];
	const order = [...spans.keys()]
		.filter((l) => l !== trunk)
		.sort((a, b) => spans.get(a)![0] - spans.get(b)![0]);
	for (const line of order) {
		const [from, to] = spans.get(line)!;
		let n = 1;
		while ((taken[n] ??= []).some(([f, t]) => from <= t && f <= to)) n++;
		taken[n].push([from, to]);
		lane.set(line, n);
	}

	const nodes: GraphNode[] = commits.map((c, i) => ({
		hash: c.hash,
		line: c.line,
		lane: lane.get(c.line) ?? 0,
		row: i,
		kind: c.pending ? 'pending' : c.parents.length > 1 ? 'interchange' : 'station'
	}));

	const edges: GraphEdge[] = [];
	commits.forEach((c, i) => {
		const a = lane.get(c.line) ?? 0;
		for (const p of c.parents) {
			const j = row.get(p);
			if (j === undefined) continue;
			const parentLine = lineOf.get(p)!;
			const b = lane.get(parentLine) ?? 0;
			let points: [number, number][];
			if (a === b)
				points = [
					[a, i],
					[b, j]
				];
			else if (a > b)
				points = [
					[a, i],
					[a, Math.max(i, j - (a - b))],
					[b, j]
				];
			else
				points = [
					[a, i],
					[b, Math.min(j, i + (b - a))],
					[b, j]
				];
			edges.push({
				from: c.hash,
				to: p,
				line: a >= b ? c.line : parentLine,
				points,
				pending: Boolean(c.pending)
			});
		}
	});
	// the trunk is drawn first so that lines joining it sit on top
	edges.sort((x, y) => Number(x.line !== trunk) - Number(y.line !== trunk));

	return { lanes: Math.max(1, taken.length), nodes, edges };
}
