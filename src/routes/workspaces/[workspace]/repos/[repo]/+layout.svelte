<script lang="ts">
	import Toolbar from '#lib/components/Toolbar.svelte';

	let { data, children } = $props();

	const session = $derived(data.session);

	// the backend says when the repository changed; the history re-reads itself
	$effect(() => {
		let cancelled = false;
		let stop: (() => void) | undefined;
		void session.listen().then((off) => {
			if (cancelled) off();
			else stop = off;
		});
		return () => {
			cancelled = true;
			stop?.();
		};
	});
</script>

<Toolbar
	workspaces={data.workspaces}
	workspace={data.workspace}
	repo={data.repo}
	worktrees={data.worktrees}
	worktree={data.worktree}
	line={session.lines[session.headLine]}
	ahead={session.ahead}
/>
<!-- a page's own state belongs to one repository -->
{#key session}
	{@render children()}
{/key}
