import { invoke, isTauri } from '@tauri-apps/api/core';
import { layoutGraph } from '#lib/graph/layout.ts';
import { repository as mock } from './mock.ts';
import {
	LINE_COLORS,
	type Commit,
	type FileChange,
	type HistoryRow,
	type HistoryWindow,
	type Repository
} from './types.ts';

/** What `open_history` returns: a repository without anything about how it is shown. */
export interface GitSummary extends Pick<
	Repository,
	'user' | 'trunk' | 'headLine' | 'head' | 'stashes' | 'tags'
> {
	ahead: number;
	lines: { id: string; name: string; remote: boolean; merged: boolean }[];
}

/** What `history_rows` returns. */
export interface GitWindow extends Omit<HistoryWindow, 'rows'> {
	rows: (Omit<HistoryRow, 'when' | 'body' | 'files'> & {
		body: string | null;
		/** Seconds since the Unix epoch. */
		time: number;
	})[];
}

/** How long ago `time` was, as short as the history's When column; both in seconds since the epoch. */
export function ago(time: number, now: number): string {
	const minutes = Math.floor((now - time) / 60);
	if (minutes < 1) return 'now';
	if (minutes < 60) return `${minutes} min`;
	if (minutes < 24 * 60) return `${Math.floor(minutes / 60)} h`;
	const days = Math.floor(minutes / (24 * 60));
	if (days === 1) return 'yesterday';
	if (days < 30) return `${days} d`;
	return new Date(time * 1000).toISOString().slice(0, 10);
}

/**
 * One letter per branch name, in order: the first letter of its last path segment that no earlier
 * line took, so `feat/graph` and `fix/gitmoji` are G and I.
 */
export function badges(names: string[]): string[] {
	const taken = new Set<string>();
	return names.map((name) => {
		const letters = (name.split('/').at(-1) ?? '').toUpperCase().match(/[\p{L}\p{N}]/gu) ?? ['?'];
		const badge = letters.find((letter) => !taken.has(letter)) ?? letters[0]!;
		taken.add(badge);
		return badge;
	});
}

/** Give a repository read from git the badges and colours of its lines. */
export function toRepository(summary: GitSummary): Repository {
	const letters = badges(summary.lines.map((line) => line.name));
	return {
		...summary,
		lines: Object.fromEntries(
			summary.lines.map((line, index) => [
				line.id,
				{ ...line, badge: letters[index]!, color: LINE_COLORS[index % LINE_COLORS.length]! }
			])
		),
		// shortcut: the working tree is not read yet, so a real repository never shows working changes
		changes: []
	};
}

/** Give rows read from git their relative times. `now` is in milliseconds. */
export function toWindow(window: GitWindow, now: number): HistoryWindow {
	return {
		...window,
		rows: window.rows.map(({ time, body, ...row }) => ({
			...row,
			body: body ?? undefined,
			when: ago(time, now / 1000)
		}))
	};
}

/**
 * Open the repository in the directory `path`; `repo` is its id in the registry. Outside the
 * Tauri shell, a synthetic one.
 */
export async function readRepository(repo: string, path: string): Promise<Repository> {
	if (!isTauri()) return mock;
	return toRepository(await invoke<GitSummary>('open_history', { repo, path }));
}

/** The rows `from` up to `to` of an opened repository's history, masked to the lines `only` unless that is empty. */
export async function historyRows(
	path: string,
	from: number,
	to: number,
	only: string[]
): Promise<HistoryWindow> {
	return toWindow(await invoke<GitWindow>('history_rows', { path, from, to, only }), Date.now());
}

/** The same window for commits held in the webview, which only the synthetic repository are. */
export function layoutWindow(
	commits: Commit[],
	trunk: string,
	from: number,
	to: number
): HistoryWindow {
	const layout = layoutGraph(commits, trunk);
	const row = new Map(commits.map((commit, index) => [commit.hash, index]));
	return {
		total: commits.length,
		lanes: layout.lanes,
		rows: commits
			.map((commit, index) => ({ ...commit, row: index, lane: layout.nodes[index]!.lane }))
			.slice(from, to),
		edges: layout.edges
			.map((edge) => ({ ...edge, from: row.get(edge.from)!, to: row.get(edge.to)! }))
			.filter((edge) => edge.from < to && edge.to >= from)
	};
}

/** The files a commit changed, read on demand. */
export async function commitFiles(path: string, commit: Commit): Promise<FileChange[]> {
	return commit.files ?? (isTauri() ? invoke('commit_files', { path, hash: commit.hash }) : []);
}
