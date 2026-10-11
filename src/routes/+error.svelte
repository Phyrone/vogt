<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import TitleBar from '#lib/components/TitleBar.svelte';
</script>

<TitleBar />
<main class="flex flex-1 flex-col items-center justify-center gap-4 p-6 text-center">
	<p class="caps text-base-content/60">{page.status}</p>
	<h1 class="signage text-2xl">{page.error?.message}</h1>
	<a class="btn btn-neutral btn-sm" href={resolve('/')}>Back to your workspaces</a>
</main>
