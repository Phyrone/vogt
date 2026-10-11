<script lang="ts">
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import Settings from '@lucide/svelte/icons/settings';
	import { page } from '$app/state';
	import { appSettingsPath, repoSettingsPath } from '#lib/settings/links.ts';
	import Switcher from './Switcher.svelte';

	const app = appSettingsPath();
	const here = $derived(page.url.pathname);
	const workspace = $derived(page.params.workspace);
	const repo = $derived(page.params.repo);
	// a repository's settings keep the worktree the repository is shown through
	const repoSettings = $derived(
		workspace && repo
			? repoSettingsPath(workspace, repo, page.url.searchParams.get('worktree'))
			: null
	);
	const repoBase = $derived(repoSettings?.split('?')[0] ?? null);
</script>

<Switcher align="end" title="Settings">
	{#snippet label()}
		<Settings class="size-4" aria-hidden="true" />
		<span class="hidden lg:inline">Settings</span>
		<span class="sr-only lg:hidden">Settings</span>
	{/snippet}
	<ul class="menu w-full p-1">
		<li>
			<a
				href={app}
				class={[here === app && 'menu-active']}
				aria-current={here === app ? 'page' : undefined}
			>
				<Settings class="size-4 opacity-70" aria-hidden="true" />App settings
			</a>
		</li>
		{#if repoSettings && repoBase}
			<li>
				<a
					href={repoSettings}
					class={[here === repoBase && 'menu-active']}
					aria-current={here === repoBase ? 'page' : undefined}
				>
					<FolderGit2 class="size-4 opacity-70" aria-hidden="true" />Repository settings
				</a>
			</li>
		{/if}
	</ul>
</Switcher>
