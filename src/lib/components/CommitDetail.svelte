<script lang="ts">
	import Cherry from '@lucide/svelte/icons/cherry';
	import GitCommitHorizontal from '@lucide/svelte/icons/git-commit-horizontal';
	import ListRestart from '@lucide/svelte/icons/list-restart';
	import PencilLine from '@lucide/svelte/icons/pencil-line';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import type { Commit, Line } from '#lib/repo/types.ts';
	import LineBadge from './LineBadge.svelte';

	interface Props {
		commit: Commit;
		line: Line;
		isHead: boolean;
	}

	let { commit, line, isHead }: Props = $props();
</script>

<article class="flex flex-col gap-3 p-4">
	<p class="flex flex-wrap items-center gap-x-2 gap-y-1 text-base-content/70">
		<LineBadge {line} size="sm" />
		<span class="font-mono text-xs">{commit.hash}</span>
		<span>{commit.author}, {commit.when}</span>
		{#if !commit.pushed}<span class="badge badge-outline signage badge-sm">not pushed</span>{/if}
	</p>
	<h2 class="signage text-xl leading-snug font-semibold text-balance">{commit.subject}</h2>
	{#if commit.body}<p class="max-w-prose text-base-content/80">{commit.body}</p>{/if}
	<div class="flex flex-wrap gap-1.5">
		{#if isHead}<button type="button" class="btn btn-sm"
				><GitCommitHorizontal class="size-4" aria-hidden="true" />Amend</button
			>{/if}
		<button type="button" class="btn btn-sm"
			><PencilLine class="size-4" aria-hidden="true" />Reword</button
		>
		<button type="button" class="btn btn-sm"
			><Cherry class="size-4" aria-hidden="true" />Cherry-pick</button
		>
		<button type="button" class="btn btn-sm"
			><RotateCcw class="size-4" aria-hidden="true" />Revert</button
		>
		<button type="button" class="btn btn-sm"
			><ListRestart class="size-4" aria-hidden="true" />Rebase from here</button
		>
	</div>
	{#if commit.files.length}
		<h3 class="mt-3 caps text-base-content/70">
			{commit.files.length}
			{commit.files.length === 1 ? 'file' : 'files'} changed
		</h3>
		<ul>
			{#each commit.files as file (file.path)}
				<li class="flex h-8 items-center gap-2.5 border-b border-base-300">
					<span class="min-w-0 flex-1 truncate" title={file.path}>{file.path}</span>
					<span class="font-mono text-xs text-success">+{file.added}</span>
					<span class="font-mono text-xs text-error">−{file.removed}</span>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="mt-3 text-base-content/70">
			This merge brought the two lines together without changes of its own.
		</p>
	{/if}
</article>
