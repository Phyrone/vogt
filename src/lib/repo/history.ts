import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Opened as GitSummary } from '#lib/bindings/Opened.ts';
import type { Progress } from '#lib/bindings/Progress.ts';
import type { ViewInfo } from '#lib/bindings/ViewInfo.ts';
import { loadRepoDatabase, type RepoDatabase, type RepoEdge, type RepoRow } from '#lib/db/repo.ts';
import { layoutGraph } from '#lib/graph/layout.ts';
import { m } from '#lib/paraglide/messages.js';
import { repository as mock } from './mock.ts';
import {
	LINE_COLORS,
	type Commit,
	type FileChange,
	type HistoryEdge,
	type HistoryRow,
	type HistoryWindow,
	type Ref,
	type Repository
} from './types.ts';

/** An opened repository: what the interface draws, and where its rows live. */
export interface Opened {
	repository: Repository;
	/** The cache database, or null for the synthetic repository. */
	db: RepoDatabase | null;
	/** The unmasked view, or null for the synthetic repository. */
	view: ViewInfo | null;
	progress: Progress;
}

/** How long ago `time` was, as short as the history's When column; both in seconds since the epoch. */
export function ago(time: number, now: number): string {
	const minutes = Math.floor((now - time) / 60);
	if (minutes < 1) return m['history.now']();
	if (minutes < 60) return m['history.minutes']({ count: minutes });
	if (minutes < 24 * 60) return m['history.hours']({ count: Math.floor(minutes / 60) });
	const days = Math.floor(minutes / (24 * 60));
	if (days === 1) return m['history.yesterday']();
	if (days < 30) return m['history.days']({ count: days });
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

/** One row as the interface draws it. `now` is in milliseconds. */
export function toHistoryRow(row: RepoRow, now: number): HistoryRow {
	return {
		hash: row.hash,
		subject: row.subject,
		body: row.body ?? undefined,
		author: row.author,
		when: ago(row.time, now / 1000),
		line: row.line,
		parents: JSON.parse(row.parents) as string[],
		refs: JSON.parse(row.refs) as Ref[],
		pushed: Boolean(row.pushed),
		row: row.row,
		lane: row.lane
	};
}

/** One edge as the graph draws it. */
export function toHistoryEdge(edge: RepoEdge): HistoryEdge {
	return {
		from: edge.from,
		to: edge.to,
		line: edge.line,
		points: JSON.parse(edge.points) as [number, number][]
	};
}

/**
 * Open the repository in the directory `path`; `repo` is its id in the registry, `workspace` the
 * id of the workspace that holds it. Outside the Tauri shell, a synthetic one.
 */
export async function readRepository(
	workspace: string,
	repo: string,
	path: string
): Promise<Opened> {
	if (!isTauri()) {
		const commits = mock.commits ?? [];
		return {
			repository: mock,
			db: null,
			view: null,
			progress: { indexed: commits.length, laid: commits.length, complete: true }
		};
	}
	const summary = await invoke<GitSummary>('open_history', { workspace, repo, path });
	return {
		repository: toRepository(summary),
		db: await loadRepoDatabase(repo, summary.cachePath),
		view: summary.view,
		progress: summary.progress
	};
}

/** Lays the picked lines out as a view of their own, and answers with it. */
export async function materializeView(repo: string, only: string[]): Promise<ViewInfo> {
	return invoke<ViewInfo>('materialize_view', { repo, only });
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
