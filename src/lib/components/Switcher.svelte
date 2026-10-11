<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import type { Snippet } from 'svelte';
	import { afterNavigate } from '$app/navigation';

	interface Props {
		/** What the button shows: the current choice. */
		label: Snippet;
		/** The choices. */
		children: Snippet;
	}

	let { label, children }: Props = $props();

	let open = $state(false);
	let root: HTMLDetailsElement;

	afterNavigate(() => {
		open = false;
	});
</script>

<svelte:window
	onclick={(event) => {
		if (open && !root.contains(event.target as Node)) open = false;
	}}
	onkeydown={(event) => {
		if (event.key === 'Escape') open = false;
	}}
/>

<details class="dropdown" bind:open bind:this={root}>
	<summary class="btn btn-ghost font-semibold btn-sm"
		>{@render label()}<ChevronDown class="size-3.5 opacity-60" aria-hidden="true" /></summary
	>
	<div
		class="dropdown-content z-10 mt-1 w-64 rounded-box border border-base-300 bg-base-100 p-1 font-normal shadow-lg"
	>
		{@render children()}
	</div>
</details>
