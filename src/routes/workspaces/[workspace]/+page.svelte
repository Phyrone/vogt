<script lang="ts">
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import Plus from '@lucide/svelte/icons/plus';
	import X from '@lucide/svelte/icons/x';
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import NewRepository from '#lib/components/NewRepository.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { addRepo, pickDirectory, REGISTRY, removeRepo } from '#lib/registry/api.ts';

	let { data } = $props();

	const workspace = $derived(data.workspace);
	let failure = $state('');
	let drafting = $state(false);

	async function add() {
		failure = '';
		try {
			const path = await pickDirectory();
			if (!path) return;
			const repo = await addRepo(workspace.id, path);
			await invalidate(REGISTRY);
			await goto(
				resolve('/workspaces/[workspace]/repos/[repo]/history', {
					workspace: workspace.id,
					repo: repo.id
				})
			);
		} catch (reason) {
			failure = String(reason);
		}
	}

	async function remove(repo: string) {
		failure = '';
		try {
			await removeRepo(workspace.id, repo);
			await invalidate(REGISTRY);
		} catch (reason) {
			failure = String(reason);
		}
	}
</script>

<Toolbar workspaces={data.workspaces} {workspace} />
<main class="min-h-0 flex-1 overflow-y-auto">
	<div class="mx-auto flex max-w-3xl flex-col gap-5 px-6 py-10">
		<div class="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
			<h1 class="min-w-0 truncate signage text-3xl">{workspace.name}</h1>
			<div class="flex gap-2">
				<button
					type="button"
					class={['btn btn-sm', drafting && 'btn-active']}
					aria-expanded={drafting}
					onclick={() => (drafting = !drafting)}
					><Plus class="size-4" aria-hidden="true" />{m['workspace.new_repository']()}</button
				>
				<button type="button" class="btn btn-neutral btn-sm" onclick={add}
					><FolderOpen class="size-4" aria-hidden="true" />{m['workspace.add_existing']()}</button
				>
			</div>
		</div>
		{#if failure}
			<div role="alert" class="alert alert-soft alert-error">{failure}</div>
		{/if}
		{#if workspace.repos.length || drafting}
			<ul
				class="list rounded-box border border-base-300"
				aria-label={m['workspace.repositories']()}
			>
				{#if drafting}
					<NewRepository workspace={workspace.id} oncancel={() => (drafting = false)} />
				{/if}
				{#each workspace.repos as repo (repo.id)}
					<li class="list-row items-center py-0 pr-2">
						<FolderGit2 class="size-5 opacity-60" aria-hidden="true" />
						<a
							class="min-w-0 py-3 -outline-offset-2"
							href={resolve('/workspaces/[workspace]/repos/[repo]/history', {
								workspace: workspace.id,
								repo: repo.id
							})}
						>
							<span class="block truncate signage text-base leading-tight">{repo.name}</span>
							<span class="block truncate font-mono text-xs text-base-content/65">{repo.path}</span>
						</a>
						<button
							type="button"
							class="btn btn-square btn-ghost btn-sm"
							title={m['common.remove_from_workspace']()}
							aria-label={m['common.remove_named_from_workspace']({ name: repo.name })}
							onclick={() => remove(repo.id)}><X class="size-4" aria-hidden="true" /></button
						>
					</li>
				{/each}
			</ul>
		{:else}
			<p
				class="rounded-box border border-dashed border-base-300 px-6 py-10 text-center text-balance"
			>
				{m['workspace.empty']()}
			</p>
		{/if}
	</div>
</main>
