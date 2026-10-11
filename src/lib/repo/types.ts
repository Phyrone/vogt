/** daisyUI theme roles a line can be drawn in, in the order lines take them. */
export const LINE_COLORS = [
	'primary',
	'secondary',
	'accent',
	'warning',
	'info',
	'success'
] as const;
export type LineColor = (typeof LINE_COLORS)[number];

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
	files: FileChange[];
	/** Uncommitted work shown as the next station on the checked-out line. */
	pending?: boolean;
}

export interface Repository {
	name: string;
	user: string;
	/** Id of the line history is laid out around. */
	trunk: string;
	/** Id of the checked-out line. */
	headLine: string;
	/** Hash the checked-out line points at. */
	head: string;
	lines: Record<string, Line>;
	/** Newest first. */
	commits: Commit[];
	changes: FileChange[];
	stashes: string[];
	tags: string[];
}

export function lineStyle(line: Line): string {
	return `--line: var(--color-${line.color}); --line-content: var(--color-${line.color}-content)`;
}
