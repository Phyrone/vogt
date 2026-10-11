import { repository } from './mock.ts';
import type { Commit, FileChange } from './types.ts';

export const WORKING = 'working-changes';

/**
 * The state of one open repository, shared by the repository's toolbar and its pages.
 *
 * Every repository shows the same synthetic contents until the backend reads real ones.
 */
export class RepoSession {
	readonly lines = repository.lines;
	readonly trunk = repository.trunk;
	readonly headLine = repository.headLine;
	readonly stashes = repository.stashes;
	readonly tags = repository.tags;

	commits = $state([...repository.commits]);
	changes: FileChange[] = $state(repository.changes.map((change) => ({ ...change })));
	head = $state(repository.head);

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
	ahead = $derived(this.commits.filter((c) => !c.pushed).length);
	scopes = $derived(
		this.commits
			.flatMap((c) => c.subject.match(/\(([\w-]+)\):/)?.[1] ?? [])
			.filter((scope, index, all) => all.indexOf(scope) === index)
			.slice(0, 6)
	);

	/** Commit the staged changes; returns the new hash. */
	commit(subject: string, body: string): string {
		const hash = Math.random().toString(16).slice(2, 9).padEnd(7, '0');
		this.commits.unshift({
			hash,
			subject,
			body: body || undefined,
			author: repository.user,
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
