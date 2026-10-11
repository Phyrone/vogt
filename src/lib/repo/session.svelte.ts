import { commitFiles } from './history.ts';
import type { Commit, FileChange, Repository } from './types.ts';

export const WORKING = 'working-changes';

/** The state of one open repository, shared by the repository's toolbar and its pages. */
export class RepoSession {
	readonly user: string;
	readonly lines: Repository['lines'];
	readonly trunk: string;
	readonly headLine: string;
	readonly stashes: string[];
	readonly tags: string[];

	commits: Commit[] = $state([]);
	changes: FileChange[] = $state([]);
	head = $state('');

	/** Newest first; uncommitted work, when there is any, is the first row. */
	rows = $derived.by((): Commit[] => {
		if (!this.changes.length) return this.commits;
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
		return [working, ...this.commits];
	});
	/** Commits on the checked-out line that no remote has. */
	ahead = $derived(this.commits.filter((c) => c.line === this.headLine && !c.pushed).length);
	scopes = $derived(
		this.commits
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
		this.commits = [...repository.commits];
		this.changes = repository.changes.map((change) => ({ ...change }));
		this.head = repository.head;
	}

	/** The files a commit changed. */
	files(commit: Commit): Promise<FileChange[]> {
		return commitFiles(this.path, commit);
	}

	/** Commit the staged changes; returns the new hash. */
	commit(subject: string, body: string): string {
		const hash = Math.random().toString(16).slice(2, 9).padEnd(7, '0');
		this.commits.unshift({
			hash,
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
		});
		this.head = hash;
		this.changes = this.changes.filter((c) => !c.staged);
		return hash;
	}
}
