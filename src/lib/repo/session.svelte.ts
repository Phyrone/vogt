import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { Changed } from '#lib/bindings/Changed.ts';
import type { Progress } from '#lib/bindings/Progress.ts';
import type { ViewInfo } from '#lib/bindings/ViewInfo.ts';
import { locate as repoLocate, window as repoWindow, type RepoDatabase } from '#lib/db/repo.ts';
import {
	commitFiles,
	layoutWindow,
	materializeView,
	readRepository,
	toHistoryEdge,
	toHistoryRow,
	type Opened
} from './history.ts';
import type { Commit, FileChange, HistoryWindow, Repository } from './types.ts';

export const WORKING = 'working-changes';

/** How long a failed refresh waits before it is tried once more, in milliseconds. */
const RETRY = 1000;

/** The state of one open repository, shared by the repository's toolbar and its pages. */
export class RepoSession {
	user = $state('');
	lines = $state<Repository['lines']>({});
	trunk = $state('');
	headLine = $state('');
	head = $state('');
	stashes = $state<string[]>([]);
	tags = $state<string[]>([]);
	/** Held here only for the synthetic repository; a real history is asked for with `rows`. */
	commits: Commit[] | undefined = $state();
	changes: FileChange[] = $state([]);
	/** How far the backend's index is. */
	progress: Progress = $state({ indexed: 0, laid: 0, complete: true });
	/** The unmasked view of a real history. */
	view: ViewInfo | null = $state(null);
	/** Goes up whenever the history changed and has to be asked for again. */
	version = $state(0);
	#ahead = $state<number | undefined>(undefined);
	#db: RepoDatabase | null;
	/** The changed version a refresh is already running for. */
	#asked = 0;
	/** The masked views asked for at `#viewsOf`, by their lines, so a mask is laid out once. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping nothing is drawn from
	#views = new Map<string, Promise<ViewInfo>>();
	#viewsOf = 0;

	/** The synthetic history: newest first; uncommitted work, when there is any, is the first row. */
	#listed = $derived.by((): Commit[] => {
		const commits = this.commits ?? [];
		if (!this.changes.length) return commits;
		const working: Commit = {
			hash: WORKING,
			subject: `Working changes · ${this.changes.length} ${this.changes.length === 1 ? 'file' : 'files'}, not committed`,
			author: '',
			when: 'now',
			line: this.headLine,
			parents: [this.head],
			pushed: false,
			files: this.changes,
			pending: true
		};
		return [working, ...commits];
	});
	#unpushed = $derived(
		(this.commits ?? []).filter((c) => c.line === this.headLine && !c.pushed).length
	);
	scopes = $derived(
		(this.commits ?? [])
			.flatMap((c) => c.subject.match(/\(([\w-]+)\):/)?.[1] ?? [])
			.filter((scope, index, all) => all.indexOf(scope) === index)
			.slice(0, 6)
	);

	/** `workspaceId` and `repoId` are the registry's ids; `path` is the directory it was read from. */
	constructor(
		opened: Opened,
		readonly workspaceId: string,
		readonly repoId: string,
		readonly path: string
	) {
		this.#db = opened.db;
		this.#apply(opened);
	}

	/** Commits on the checked-out line that no remote has. */
	get ahead(): number {
		return this.#ahead ?? this.#unpushed;
	}

	/** Takes over everything a freshly opened repository carries. */
	#apply(opened: Opened) {
		this.user = opened.repository.user;
		this.lines = opened.repository.lines;
		this.trunk = opened.repository.trunk;
		this.headLine = opened.repository.headLine;
		this.head = opened.repository.head;
		this.stashes = opened.repository.stashes;
		this.tags = opened.repository.tags;
		this.#ahead = opened.repository.ahead;
		this.commits = opened.repository.commits && [...opened.repository.commits];
		this.changes = opened.repository.changes.map((change) => ({ ...change }));
		this.view = opened.view;
		this.progress = opened.progress;
		this.version = opened.view?.version ?? 0;
	}

	/** Listens for the backend's updates to this repository; returns how to stop. */
	async listen(): Promise<UnlistenFn> {
		if (!isTauri()) return () => {};
		let retry: ReturnType<typeof setTimeout> | undefined;
		const reread = (asked: number, retried = false) => {
			this.#asked = asked;
			this.refresh().catch((reason) => {
				console.error('vogt: the history could not be read again', reason);
				if (this.#asked !== asked) return;
				// the backend sends one last event a round, so unless it is tried again here the history
				// stays as it was until the refs move; when that try fails too, the next event asks
				if (retried) {
					this.#asked = 0;
					return;
				}
				retry = setTimeout(() => {
					if (this.#asked === asked) reread(asked, true);
				}, RETRY);
			});
		};
		const unlisten = await listen<Changed>('repo-changed', (event) => {
			const changed = event.payload;
			if (changed.repo !== this.repoId) return;
			this.progress = {
				indexed: changed.indexed,
				laid: changed.laid,
				complete: changed.complete
			};
			// `version` moves only with `lines`, in #apply; this keeps one refresh
			// per version while it is still running
			if (changed.version === this.version || changed.version === this.#asked) return;
			reread(changed.version);
		});
		return () => {
			clearTimeout(retry);
			// a refresh still on its way is not tried again, and whoever listens next asks anew
			this.#asked = 0;
			unlisten();
		};
	}

	/** Re-reads the summary after the cache moved on. */
	async refresh(): Promise<void> {
		// the synthetic repository holds its own history and has nothing to re-read
		if (this.commits) return;
		this.#apply(await readRepository(this.workspaceId, this.repoId, this.path));
	}

	/** The rows `from` up to `to` of the history, masked to the lines `only` unless that is empty. */
	async rows(from: number, to: number, only: string[]): Promise<HistoryWindow> {
		if (this.commits) {
			const listed = only.length
				? this.#listed.filter((commit) => only.includes(commit.line))
				: this.#listed;
			return layoutWindow(listed, this.trunk, Object.keys(this.lines), from, to);
		}
		if (!this.#db) throw new Error('This repository’s cache is not open.');
		const view = only.length ? await this.#masked(only) : this.view;
		if (!view) throw new Error('This repository’s history is not laid out yet.');
		const { rows, edges } = await repoWindow(this.#db, view, from, to);
		const now = Date.now();
		return {
			total: view.total,
			lanes: view.lanes,
			rows: rows.map((row) => toHistoryRow(row, now)),
			edges: edges.map(toHistoryEdge)
		};
	}

	/** The view of the lines `only`; the chunks of one paint ask for it at the same time. */
	#masked(only: string[]): Promise<ViewInfo> {
		if (this.#viewsOf !== this.version) {
			this.#views.clear();
			this.#viewsOf = this.version;
		}
		const key = [...only]
			.sort()
			.filter((line, index, all) => line !== all[index - 1])
			.join('\n');
		let view = this.#views.get(key);
		if (!view) {
			const asked = materializeView(this.repoId, only);
			this.#views.set(key, asked);
			asked.catch(() => {
				if (this.#views.get(key) === asked) this.#views.delete(key);
			});
			view = asked;
		}
		return view;
	}

	/** The row the commit `hash` sits in, of the unmasked view. */
	async locate(hash: string): Promise<number | null> {
		return this.#db ? repoLocate(this.#db, hash) : null;
	}

	/** The files a commit changed. */
	files(commit: Commit): Promise<FileChange[]> {
		return commitFiles(this.path, commit);
	}

	/** Commit the staged changes of the synthetic repository; returns the new commit. */
	commit(subject: string, body: string): Commit {
		const made: Commit = {
			hash: Math.random().toString(16).slice(2, 9).padEnd(7, '0'),
			subject,
			body: body || undefined,
			author: this.user,
			when: 'now',
			line: this.headLine,
			parents: [this.head],
			pushed: false,
			files: this.changes
				.filter((c) => c.staged)
				.map(({ path, added, removed }) => ({ path, added, removed }))
		};
		this.commits = [made, ...(this.commits ?? [])];
		this.head = made.hash;
		this.changes = this.changes.filter((c) => !c.staged);
		this.version++;
		return made;
	}
}
