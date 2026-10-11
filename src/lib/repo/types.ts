/** daisyUI theme roles a line can be drawn in, in the order lines take them. */
export const LINE_COLORS = [
	'primary',
	'secondary',
	'accent',
	'warning',
	'info',
	'success'
] as const;
export type LineColor = (typeof LINE_COLORS)[number] | 'neutral';

/** A branch as the interface shows it: a named line with a lettered badge. */
export interface Line {
	id: string;
	name: string;
	badge: string;
	color: LineColor;
	/** Has a counterpart on the remote. */
	remote: boolean;
	/** Already merged and deleted; drawn in the graph but absent from the sidebar. */
	merged?: boolean;
}

export interface FileChange {
	path: string;
	added: number;
	removed: number;
	staged?: boolean;
}

export interface Ref {
	kind: 'remote' | 'tag';
	label: string;
}

export interface Commit {
	hash: string;
	subject: string;
	body?: string;
	author: string;
	when: string;
	/** Id of the line this commit sits on. */
	line: string;
	/** Parent hashes, first parent first. */
	parents: string[];
	refs?: Ref[];
	pushed: boolean;
	/** Absent while they have not been read; see `commitFiles`. */
	files?: FileChange[];
	/** Uncommitted work shown as the next station on the checked-out line. */
	pending?: boolean;
}

export interface Repository {
	user: string;
	/** Id of the line history is laid out around. */
	trunk: string;
	/** Id of the checked-out line. */
	headLine: string;
	/** Hash the checked-out line points at; empty before the first commit. */
	head: string;
	lines: Record<string, Line>;
	/** Commits on the checked-out line that no remote has. Counted from `commits` when absent. */
	ahead?: number;
	/**
	 * Newest first. Only the synthetic repository holds its commits here; a real history stays in
	 * the backend and is asked for a window at a time.
	 */
	commits?: Commit[];
	changes: FileChange[];
	stashes: string[];
	tags: string[];
}

/** A commit as a row of the history: where it sits in the list and in the graph. */
export interface HistoryRow extends Commit {
	row: number;
	lane: number;
}

/** A stretch of a line between two commits. */
export interface HistoryEdge {
	/** Rows of the child and the parent. */
	from: number;
	to: number;
	/** Id of the line whose colour the edge takes: the one that leaves or joins the other. */
	line: string;
	/** Corner points as [lane, row]. */
	points: [number, number][];
	pending?: boolean;
}

/** Some rows of a history and the edges that cross them. */
export interface HistoryWindow {
	/** How many rows the history has. */
	total: number;
	/** How many lanes the whole graph has. */
	lanes: number;
	rows: HistoryRow[];
	edges: HistoryEdge[];
}

export function lineStyle(line: Line): string {
	return `--line: var(--color-${line.color}); --line-content: var(--color-${line.color}-content)`;
}

/**
 * The line a commit is drawn with; a row can arrive before its line does, so a
 * missing one falls back to a neutral badge named after its id.
 */
export function lineOf(lines: Record<string, Line>, id: string): Line {
	return (
		lines[id] ?? {
			id,
			name: id,
			badge: '?',
			color: 'neutral',
			remote: false
		}
	);
}
