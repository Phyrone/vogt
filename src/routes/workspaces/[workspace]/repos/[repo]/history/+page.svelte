<script lang="ts">
	import CommitComposer from '#lib/components/CommitComposer.svelte';
	import CommitDetail from '#lib/components/CommitDetail.svelte';
	import History from '#lib/components/History.svelte';
	import RefSidebar from '#lib/components/RefSidebar.svelte';
	import type { Commit } from '#lib/repo/types.ts';

	let { data } = $props();

	const session = $derived(data.session);
	const { lines, headLine } = $derived(session);

	// nothing selected is the working changes, or the checked-out commit once the history has it
	let selected = $state<Commit | null>(null);
	// the lines picked in the sidebar: a mask over the history, not a place to go
	let picked = $state<string[]>([]);

	function oncommit(subject: string, body: string) {
		const made = session.commit(subject, body);
		if (!session.changes.length) selected = made;
	}
</script>

<div class="flex min-h-0 flex-1">
	<RefSidebar
		lines={Object.values(lines)}
		{headLine}
		ahead={session.ahead}
		stashes={session.stashes}
		tags={session.tags}
		bind:picked
	/>
	<main class="flex min-w-0 flex-1">
		<History
			source={session}
			{lines}
			trunk={session.trunk}
			head={session.head}
			bind:selected
			bind:picked
		/>
	</main>
	<aside
		class="w-64 shrink-0 overflow-y-auto border-l border-base-300 lg:w-72 xl:w-90"
		aria-label={selected ? 'Commit' : 'Working changes'}
	>
		{#if selected}
			<CommitDetail
				commit={selected}
				files={session.files(selected)}
				line={lines[selected.line]}
				isHead={selected.hash === session.head}
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
