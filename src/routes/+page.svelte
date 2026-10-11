<script lang="ts">
	import CommitComposer from '#lib/components/CommitComposer.svelte';
	import CommitDetail from '#lib/components/CommitDetail.svelte';
	import History from '#lib/components/History.svelte';
	import RefSidebar from '#lib/components/RefSidebar.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import { repository } from '#lib/repo/mock.ts';
	import type { Commit } from '#lib/repo/types.ts';

	const WORKING = 'working-changes';
	const { lines, trunk, headLine } = repository;

	let commits = $state([...repository.commits]);
	let changes = $state(repository.changes.map((change) => ({ ...change })));
	let head = $state(repository.head);
	let selected = $state(WORKING);
	let focusLine = $state<string | null>(null);

	const rows = $derived.by((): Commit[] => {
		if (!changes.length) return commits;
		const working: Commit = {
			hash: WORKING,
			subject: `Working changes · ${changes.length} ${changes.length === 1 ? 'file' : 'files'}, not committed`,
			author: '',
			when: 'now',
			line: headLine,
			parents: [head],
			pushed: false,
			files: changes,
			pending: true
		};
		return [working, ...commits];
	});
	const ahead = $derived(commits.filter((c) => !c.pushed).length);
	const scopes = $derived(
		[...new Set(commits.flatMap((c) => c.subject.match(/\(([\w-]+)\):/)?.[1] ?? []))].slice(0, 6)
	);
	const commit = $derived(commits.find((c) => c.hash === selected));

	function oncommit(subject: string, body: string) {
		const hash = Math.random().toString(16).slice(2, 9).padEnd(7, '0');
		commits.unshift({
			hash,
			subject,
			body: body || undefined,
			author: repository.user,
			when: 'now',
			line: headLine,
			parents: [head],
			pushed: false,
			files: changes
				.filter((c) => c.staged)
				.map(({ path, added, removed }) => ({ path, added, removed }))
		});
		head = hash;
		changes = changes.filter((c) => !c.staged);
		if (!changes.length) selected = hash;
	}
</script>

<div class="flex h-dvh flex-col bg-base-100 text-sm">
	<Toolbar name={repository.name} line={lines[headLine]} {ahead} />
	<div class="flex min-h-0 flex-1">
		<RefSidebar
			lines={Object.values(lines)}
			{headLine}
			{ahead}
			stashes={repository.stashes}
			tags={repository.tags}
			bind:focusLine
		/>
		<main class="flex min-w-0 flex-1">
			<History {rows} {lines} {trunk} {head} bind:selected {focusLine} />
		</main>
		<aside
			class="w-64 shrink-0 overflow-y-auto border-l border-base-300 lg:w-72 xl:w-90"
			aria-label={commit ? 'Commit' : 'Working changes'}
		>
			{#if commit}
				<CommitDetail {commit} line={lines[commit.line]} isHead={commit.hash === head} />
			{:else if changes.length}
				<CommitComposer bind:changes line={lines[headLine]} {scopes} {oncommit} />
			{/if}
		</aside>
	</div>
</div>
