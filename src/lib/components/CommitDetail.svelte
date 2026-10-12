<script lang="ts">
	import Cherry from '@lucide/svelte/icons/cherry';
	import GitCommitHorizontal from '@lucide/svelte/icons/git-commit-horizontal';
	import ListRestart from '@lucide/svelte/icons/list-restart';
	import PencilLine from '@lucide/svelte/icons/pencil-line';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import type { Commit, FileChange, Line } from '#lib/repo/types.ts';
	import LineBadge from './LineBadge.svelte';

	interface Props {
		commit: Commit;
		files: Promise<FileChange[]>;
		line: Line;
		isHead: boolean;
	}

	let { commit, files, line, isHead }: Props = $props();

	// commit messages are wrapped by hand at 72 columns: prose is rejoined to fit the pane,
	// while lists and trailers keep their line breaks
	const paragraphs = $derived(
		(commit.body ?? '')
			.trim()
			.split(/\n{2,}/)
			.filter(Boolean)
			.map((text) => ({
				text,
				lines: text.split('\n').some((row) => /^\s*([-*+]|\d+[.)]|[\w-]+:)\s/.test(row))
			}))
	);
</script>

<article class="flex flex-col gap-3 p-4">
	<p class="flex flex-wrap items-center gap-x-2 gap-y-1 text-base-content/70">
		<LineBadge {line} size="sm" />
		<span class="font-mono text-xs">{commit.hash.slice(0, 7)}</span>
		<span>{commit.author}, {commit.when}</span>
		{#if !commit.pushed}<span class="badge badge-outline signage badge-sm">not pushed</span>{/if}
	</p>
	<h2 class="signage text-xl leading-snug font-semibold text-balance">{commit.subject}</h2>
	{#each paragraphs as paragraph (paragraph)}
		<p class={['max-w-prose text-base-content/80', paragraph.lines && 'whitespace-pre-line']}>
			{paragraph.text}
		</p>
	{/each}
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
	{#await files then changed}
		{#if changed.length}
			<h3 class="mt-3 caps text-base-content/70">
				{changed.length}
				{changed.length === 1 ? 'file' : 'files'} changed
			</h3>
			<ul>
				{#each changed as file (file.path)}
					<li class="flex h-8 items-center gap-2.5 border-b border-base-300">
						<span class="min-w-0 flex-1 truncate" title={file.path}>{file.path}</span>
						<span class="font-mono text-xs text-success">+{file.added}</span>
						<span class="font-mono text-xs text-error">−{file.removed}</span>
					</li>
				{/each}
			</ul>
		{:else if commit.parents.length > 1}
			<p class="mt-3 text-base-content/70">
				This merge brought the two lines together without changes of its own.
			</p>
		{:else}
			<p class="mt-3 text-base-content/70">This commit changed no files.</p>
		{/if}
	{:catch reason}
		<p role="alert" class="mt-3 text-error">The changed files could not be read: {reason}</p>
	{/await}
</article>
