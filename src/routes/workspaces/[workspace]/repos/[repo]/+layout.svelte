<script lang="ts">
	import Toolbar from '#lib/components/Toolbar.svelte';

	let { data, children } = $props();

	const session = $derived(data.session);
</script>

<Toolbar
	workspaces={data.workspaces}
	workspace={data.workspace}
	repo={data.repo}
	line={session.lines[session.headLine]}
	ahead={session.ahead}
/>
<!-- a page's own state belongs to one repository -->
{#key session}
	{@render children()}
{/key}
