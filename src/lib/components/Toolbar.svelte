<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import GitBranchPlus from '@lucide/svelte/icons/git-branch-plus';
	import Layers from '@lucide/svelte/icons/layers';
	import Redo2 from '@lucide/svelte/icons/redo-2';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { createWorkspace, REGISTRY } from '#lib/registry/api.ts';
	import type { Repo, Workspace } from '#lib/registry/types.ts';
	import type { Line } from '#lib/repo/types.ts';
	import LineBadge from './LineBadge.svelte';
	import Switcher from './Switcher.svelte';
	import TitleBar from './TitleBar.svelte';

	interface Props {
		workspaces: Workspace[];
		workspace: Workspace;
		/** The open repository; without one the toolbar only switches workspace and repository. */
		repo?: Repo;
		/** The checked-out line of the open repository. */
		line?: Line;
		ahead?: number;
	}

	let { workspaces, workspace, repo, line, ahead = 0 }: Props = $props();

	let newName = $state('');

	async function create(event: SubmitEvent) {
		event.preventDefault();
		const created = await createWorkspace(newName);
		newName = '';
		await invalidate(REGISTRY);
		await goto(resolve('/workspaces/[workspace]', { workspace: created.id }));
	}
</script>

<TitleBar>
	<Switcher>
		{#snippet label()}
			<Layers class="size-4" aria-hidden="true" /><span class="sr-only">Workspace:</span><span
				class="max-w-40 truncate">{workspace.name}</span
			>
		{/snippet}
		<ul class="menu w-full p-1">
			<li class="menu-title caps">Workspaces</li>
			{#each workspaces as item (item.id)}
				<li>
					<a
						href={resolve('/workspaces/[workspace]', { workspace: item.id })}
						class={[item.id === workspace.id && 'menu-active']}
						aria-current={item.id === workspace.id ? 'true' : undefined}
						><span class="truncate">{item.name}</span><span
							class="ml-auto font-mono text-xs opacity-60">{item.repos.length}</span
						></a
					>
				</li>
			{/each}
		</ul>
		<form class="join w-full border-t border-base-300 p-2" onsubmit={create}>
			<input
				class="input join-item input-sm"
				placeholder="New workspace"
				aria-label="New workspace name"
				required
				bind:value={newName}
			/>
			<button type="submit" class="btn join-item btn-sm">Create</button>
		</form>
	</Switcher>
	<span class="text-base-content/40" aria-hidden="true">/</span>
	<Switcher>
		{#snippet label()}
			<FolderGit2 class="size-4" aria-hidden="true" />
			{#if repo}
				<h1 class="max-w-40 truncate">{repo.name}</h1>
			{:else}
				<span class="font-normal text-base-content/70">Open a repository</span>
			{/if}
		{/snippet}
		<ul class="menu w-full p-1">
			<li class="menu-title caps">{workspace.name}</li>
			{#each workspace.repos as item (item.id)}
				<li>
					<a
						href={resolve('/workspaces/[workspace]/repos/[repo]/history', {
							workspace: workspace.id,
							repo: item.id
						})}
						class={[item.id === repo?.id && 'menu-active']}
						aria-current={item.id === repo?.id ? 'true' : undefined}
						><span class="truncate">{item.name}</span></a
					>
				</li>
			{/each}
			<li class={[workspace.repos.length > 0 && 'mt-1 border-t border-base-300 pt-1']}>
				<a href={resolve('/workspaces/[workspace]', { workspace: workspace.id })}
					>All repositories</a
				>
			</li>
		</ul>
	</Switcher>
	{#if repo && line}
		<span class="text-base-content/40" aria-hidden="true">/</span>
		<button type="button" class="btn btn-ghost font-semibold btn-sm"
			><LineBadge {line} size="sm" /><span class="max-w-40 truncate">{line.name}</span><ChevronDown
				class="size-3.5 opacity-60"
				aria-hidden="true"
			/></button
		>
		<span class="mx-2 h-6 w-px bg-base-300" data-tauri-drag-region></span>
		<button type="button" class="btn btn-ghost btn-sm"
			><ArrowDownToLine class="size-4" aria-hidden="true" />Pull</button
		>
		<button type="button" class={['btn btn-sm', ahead > 0 ? 'btn-neutral' : 'btn-ghost']}
			><ArrowUpFromLine class="size-4" aria-hidden="true" />Push{ahead > 0
				? ` ${ahead}`
				: ''}</button
		>
	{/if}
	{#snippet tools()}
		{#if repo}
			<div class="hidden items-center lg:flex">
				<button type="button" class="btn btn-square btn-ghost btn-sm" title="Undo" aria-label="Undo"
					><Undo2 class="size-4" aria-hidden="true" /></button
				>
				<button type="button" class="btn btn-square btn-ghost btn-sm" title="Redo" aria-label="Redo"
					><Redo2 class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title="Branch"
					aria-label="Branch"><GitBranchPlus class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title="Stash"
					aria-label="Stash"><Archive class="size-4" aria-hidden="true" /></button
				>
				<button type="button" class="btn btn-square btn-ghost btn-sm" title="Pop" aria-label="Pop"
					><ArchiveRestore class="size-4" aria-hidden="true" /></button
				>
			</div>
		{/if}
	{/snippet}
</TitleBar>
