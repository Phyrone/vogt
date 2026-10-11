import { invoke, isTauri } from '@tauri-apps/api/core';
import { repository as mock } from './mock.ts';
import { LINE_COLORS, type Commit, type FileChange, type Repository } from './types.ts';

/** What the `read_history` command returns: a repository without anything about how it is shown. */
export interface GitHistory extends Pick<
	Repository,
	'user' | 'trunk' | 'headLine' | 'head' | 'stashes' | 'tags'
> {
	lines: { id: string; name: string; remote: boolean; merged: boolean }[];
	commits: (Omit<Commit, 'when' | 'body' | 'files'> & {
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

/** Give a history read from git its badges, colours and relative times. `now` is in milliseconds. */
export function toRepository(history: GitHistory, now: number): Repository {
	const letters = badges(history.lines.map((line) => line.name));
	return {
		...history,
		lines: Object.fromEntries(
			history.lines.map((line, index) => [
				line.id,
				{ ...line, badge: letters[index]!, color: LINE_COLORS[index % LINE_COLORS.length]! }
			])
		),
		commits: history.commits.map(({ time, body, ...commit }) => ({
			...commit,
			body: body ?? undefined,
			when: ago(time, now / 1000)
		})),
		// shortcut: the working tree is not read yet, so a real repository never shows working changes
		changes: []
	};
}

/** Read the repository in the directory `path`; outside the Tauri shell, a synthetic one. */
export async function readRepository(path: string): Promise<Repository> {
	if (!isTauri()) return mock;
	return toRepository(await invoke<GitHistory>('read_history', { path }), Date.now());
}

/** The files a commit changed, read on demand. */
export async function commitFiles(path: string, commit: Commit): Promise<FileChange[]> {
	return commit.files ?? (isTauri() ? invoke('commit_files', { path, hash: commit.hash }) : []);
}
