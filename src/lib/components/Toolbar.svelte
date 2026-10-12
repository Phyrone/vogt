<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import FolderTree from '@lucide/svelte/icons/folder-tree';
	import GitBranchPlus from '@lucide/svelte/icons/git-branch-plus';
	import Layers from '@lucide/svelte/icons/layers';
	import Redo2 from '@lucide/svelte/icons/redo-2';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { m } from '#lib/paraglide/messages.js';
	import { createWorkspace, REGISTRY } from '#lib/registry/api.ts';
	import type { Repo, Workspace } from '#lib/registry/types.ts';
	import type { Line } from '#lib/repo/types.ts';
	import type { Worktree } from '#lib/repo/worktrees.ts';
	import LineBadge from './LineBadge.svelte';
	import Switcher from './Switcher.svelte';
	import TitleBar from './TitleBar.svelte';

	interface Props {
		workspaces: Workspace[];
		workspace: Workspace;
		/** The open repository; without one the toolbar only switches workspace and repository. */
		repo?: Repo;
		/** The open repository's worktrees; the switcher only shows when there is a choice. */
		worktrees?: Worktree[];
		/** The worktree the repository is shown through. */
		worktree?: Worktree;
		/** The checked-out line of the open repository. */
		line?: Line;
		ahead?: number;
	}

	let { workspaces, workspace, repo, worktrees = [], worktree, line, ahead = 0 }: Props = $props();

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
			<Layers class="size-4" aria-hidden="true" /><span class="sr-only"
				>{m['common.workspace_label']()}</span
			><span class="max-w-40 truncate">{workspace.name}</span>
		{/snippet}
		<ul class="menu w-full p-1">
			<li class="menu-title caps">{m['common.workspaces']()}</li>
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
				placeholder={m['common.new_workspace']()}
				aria-label={m['common.new_workspace_name']()}
				required
				bind:value={newName}
			/>
			<button type="submit" class="btn join-item btn-sm">{m['common.create']()}</button>
		</form>
	</Switcher>
	<span class="text-base-content/40" aria-hidden="true">/</span>
	<Switcher>
		{#snippet label()}
			<FolderGit2 class="size-4" aria-hidden="true" />
			{#if repo}
				<h1 class="max-w-40 truncate">{repo.name}</h1>
			{:else}
				<span class="font-normal text-base-content/70">{m['common.open_repository']()}</span>
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
					>{m['common.all_repositories']()}</a
				>
			</li>
		</ul>
	</Switcher>
	{#if repo && worktree && worktrees.length > 1}
		<span class="text-base-content/40" aria-hidden="true">/</span>
		<Switcher>
			{#snippet label()}
				<FolderTree class="size-4" aria-hidden="true" /><span class="sr-only"
					>{m['common.worktree_label']()}</span
				><span class="max-w-40 truncate">{worktree.name}</span>
			{/snippet}
			<ul class="menu w-full p-1">
				<li class="menu-title caps">{m['common.worktrees']()}</li>
				{#each worktrees as item (item.path)}
					<li>
						<a
							href="{resolve('/workspaces/[workspace]/repos/[repo]/history', {
								workspace: workspace.id,
								repo: repo.id
							})}?worktree={encodeURIComponent(item.path)}"
							class={[item.path === worktree.path && 'menu-active']}
							aria-current={item.path === worktree.path ? 'true' : undefined}
							><span class="truncate">{item.name}</span><span
								class="ml-auto max-w-28 truncate font-mono text-xs opacity-60"
								>{item.branch ?? m['common.detached']()}</span
							></a
						>
					</li>
				{/each}
			</ul>
		</Switcher>
	{/if}
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
			><ArrowDownToLine class="size-4" aria-hidden="true" />{m['common.pull']()}</button
		>
		<button type="button" class={['btn btn-sm', ahead > 0 ? 'btn-neutral' : 'btn-ghost']}
			><ArrowUpFromLine class="size-4" aria-hidden="true" />{m['common.push']()}{ahead > 0
				? ` ${ahead}`
				: ''}</button
		>
	{/if}
	{#snippet tools()}
		{#if repo}
			<div class="hidden items-center lg:flex">
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title={m['common.undo']()}
					aria-label={m['common.undo']()}><Undo2 class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title={m['common.redo']()}
					aria-label={m['common.redo']()}><Redo2 class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title={m['common.branch']()}
					aria-label={m['common.branch']()}
					><GitBranchPlus class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title={m['common.stash']()}
					aria-label={m['common.stash']()}><Archive class="size-4" aria-hidden="true" /></button
				>
				<button
					type="button"
					class="btn btn-square btn-ghost btn-sm"
					title={m['common.pop']()}
					aria-label={m['common.pop']()}
					><ArchiveRestore class="size-4" aria-hidden="true" /></button
				>
			</div>
		{/if}
	{/snippet}
</TitleBar>
