<script lang="ts">
	import { untrack } from 'svelte';
	import CommitComposer from '#lib/components/CommitComposer.svelte';
	import CommitDetail from '#lib/components/CommitDetail.svelte';
	import History from '#lib/components/History.svelte';
	import RefSidebar from '#lib/components/RefSidebar.svelte';
	import { WORKING } from '#lib/repo/session.svelte.ts';

	let { data } = $props();

	const session = $derived(data.session);
	const { lines, headLine } = $derived(session);

	let selected = $state(untrack(() => (session.changes.length ? WORKING : session.head)));
	let focusLine = $state<string | null>(null);

	const commit = $derived(session.commits.find((c) => c.hash === selected));

	function oncommit(subject: string, body: string) {
		const hash = session.commit(subject, body);
		if (!session.changes.length) selected = hash;
	}
</script>

<div class="flex min-h-0 flex-1">
	<RefSidebar
		lines={Object.values(lines)}
		{headLine}
		ahead={session.ahead}
		stashes={session.stashes}
		tags={session.tags}
		bind:focusLine
	/>
	<main class="flex min-w-0 flex-1">
		<History
			rows={session.rows}
			{lines}
			trunk={session.trunk}
			head={session.head}
			bind:selected
			{focusLine}
		/>
	</main>
	<aside
		class="w-64 shrink-0 overflow-y-auto border-l border-base-300 lg:w-72 xl:w-90"
		aria-label={commit ? 'Commit' : 'Working changes'}
	>
		{#if commit}
			<CommitDetail
				{commit}
				files={session.files(commit)}
				line={lines[commit.line]}
				isHead={commit.hash === session.head}
			/>
		{:else if session.changes.length}
			<CommitComposer
				bind:changes={session.changes}
				line={lines[headLine]}
				scopes={session.scopes}
				{oncommit}
			/>
		{/if}
	</aside>
</div>
