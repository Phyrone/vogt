<script lang="ts">
	import FolderPlus from '@lucide/svelte/icons/folder-plus';
	import X from '@lucide/svelte/icons/x';
	import { goto, invalidate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { addRepo, pickDirectory, REGISTRY } from '#lib/registry/api.ts';
	import { createRepository } from '#lib/repo/create.ts';

	interface Props {
		/** Id of the workspace the repository is added to. */
		workspace: string;
		oncancel: () => void;
	}

	let { workspace, oncancel }: Props = $props();

	let name = $state('');
	let location = $state('');
	let failure = $state('');
	let creating = $state(false);

	async function choose() {
		location = (await pickDirectory()) ?? location;
	}

	async function create(event: SubmitEvent) {
		event.preventDefault();
		failure = '';
		// Enter on the name is enough: the location is asked for on the way
		if (!location) await choose();
		if (!location) return;
		creating = true;
		try {
			const repo = await addRepo(workspace, await createRepository(location, name));
			await invalidate(REGISTRY);
			await goto(
				resolve('/workspaces/[workspace]/repos/[repo]/history', { workspace, repo: repo.id })
			);
		} catch (reason) {
			failure = String(reason);
			creating = false;
		}
	}
</script>

<!-- the form is drawn as the list row it is about to become -->
<li
	class="list-row items-center bg-base-200 py-0 pr-2 -outline-offset-2 outline-base-content has-[input:focus-visible]:outline-2"
>
	<FolderPlus class="size-5 opacity-60" aria-hidden="true" />
	<form id="new-repository" class="min-w-0 py-2" onsubmit={create}>
		<input
			class="w-full bg-transparent signage text-base leading-tight placeholder:font-semibold placeholder:text-base-content/55 focus-visible:outline-none"
			placeholder="Name the new repository"
			aria-label="Name of the new repository"
			aria-describedby={failure ? 'new-repository-failure' : undefined}
			autocomplete="off"
			spellcheck="false"
			required
			bind:value={name}
			onkeydown={(event) => {
				if (event.key === 'Escape') oncancel();
			}}
			{@attach (input) => input.focus()}
		/>
		<p class="flex min-w-0 font-mono text-xs text-base-content/65">
			<button
				type="button"
				class={[
					'cursor-pointer truncate underline decoration-dotted underline-offset-2 hover:text-base-content hover:decoration-solid',
					!location && 'font-sans'
				]}
				onclick={choose}>{location || 'Choose where to put it'}</button
			>{#if location}<span class="shrink-0">/{name.trim() || '…'}</span>{/if}
		</p>
	</form>
	<div class="flex items-center gap-1">
		<button
			type="submit"
			form="new-repository"
			class="btn btn-neutral btn-sm"
			disabled={!name.trim() || creating}
			>{#if creating}<span class="loading loading-xs loading-spinner" aria-hidden="true"
				></span>{/if}Create</button
		>
		<button
			type="button"
			class="btn btn-square btn-ghost btn-sm"
			title="Cancel"
			aria-label="Cancel the new repository"
			onclick={oncancel}><X class="size-4" aria-hidden="true" /></button
		>
	</div>
	{#if failure}
		<p id="new-repository-failure" role="alert" class="list-col-wrap pb-3 text-error">
			{failure}
		</p>
	{/if}
</li>
