import { commitFiles, historyRows, layoutWindow } from './history.ts';
import type { Commit, FileChange, HistoryWindow, Repository } from './types.ts';

export const WORKING = 'working-changes';

/** The state of one open repository, shared by the repository's toolbar and its pages. */
export class RepoSession {
	readonly user: string;
	readonly lines: Repository['lines'];
	readonly trunk: string;
	readonly headLine: string;
	readonly stashes: string[];
	readonly tags: string[];
	/** Counted by the backend for a real repository. */
	readonly #ahead: number | undefined;

	/** Held here only for the synthetic repository; a real history is asked for with `rows`. */
	commits: Commit[] | undefined = $state();
	changes: FileChange[] = $state([]);
	head = $state('');
	/** Goes up whenever the history changed and has to be asked for again. */
	version = $state(0);

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

	/** `path` is the directory `repository` was read from. */
	constructor(
		repository: Repository,
		readonly path: string
	) {
		this.user = repository.user;
		this.lines = repository.lines;
		this.trunk = repository.trunk;
		this.headLine = repository.headLine;
		this.stashes = repository.stashes;
		this.tags = repository.tags;
		this.#ahead = repository.ahead;
		this.commits = repository.commits && [...repository.commits];
		this.changes = repository.changes.map((change) => ({ ...change }));
		this.head = repository.head;
	}

	/** Commits on the checked-out line that no remote has. */
	get ahead(): number {
		return this.#ahead ?? this.#unpushed;
	}

	/** The rows `from` up to `to` of the history, masked to the lines `only` unless that is empty. */
	async rows(from: number, to: number, only: string[]): Promise<HistoryWindow> {
		if (!this.commits) return historyRows(this.path, from, to, only);
		const listed = only.length
			? this.#listed.filter((commit) => only.includes(commit.line))
			: this.#listed;
		return layoutWindow(listed, this.trunk, from, to);
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
