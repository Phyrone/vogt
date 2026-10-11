<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import TitleBar from '#lib/components/TitleBar.svelte';
	import { appSettings } from '#lib/settings/app.svelte.ts';
	import {
		appCachePath,
		appDataPath,
		appVersion,
		checkForUpdates,
		desktop,
		openCacheDir,
		openDataDir,
		openExternal,
		rebuildCaches
	} from '#lib/settings/system.ts';

	// every theme daisyUI ships; the interface takes all of its colour from the active one
	const themes = [
		'light',
		'dark',
		'cupcake',
		'bumblebee',
		'emerald',
		'corporate',
		'synthwave',
		'retro',
		'cyberpunk',
		'valentine',
		'halloween',
		'garden',
		'forest',
		'aqua',
		'lofi',
		'pastel',
		'fantasy',
		'wireframe',
		'black',
		'luxury',
		'dracula',
		'cmyk',
		'autumn',
		'business',
		'acid',
		'lemonade',
		'night',
		'coffee',
		'winter',
		'dim',
		'nord',
		'sunset',
		'caramellatte',
		'abyss',
		'silk'
	];
	// translations are on the way; the choice is kept for when they arrive
	const languages = [
		{ id: 'en', name: 'English' },
		{ id: 'de', name: 'Deutsch' }
	];
	const REPOSITORY = 'https://github.com/Phyrone/vogt';
	const ISSUES = 'https://github.com/Phyrone/vogt/issues';

	let storageNotice = $state('');
	let updateNotice = $state('');

	async function open(action: () => Promise<void>): Promise<void> {
		storageNotice = '';
		try {
			await action();
		} catch (reason) {
			storageNotice = String(reason);
		}
	}

	async function rebuild(): Promise<void> {
		storageNotice = '';
		try {
			await rebuildCaches();
		} catch (reason) {
			storageNotice = String(reason);
		}
	}

	async function check(): Promise<void> {
		updateNotice = '';
		try {
			await checkForUpdates();
		} catch (reason) {
			updateNotice = String(reason);
		}
	}

	function follow(event: MouseEvent, url: string): void {
		event.preventDefault();
		void openExternal(url);
	}
</script>

<TitleBar>
	<button type="button" class="btn btn-ghost btn-sm" onclick={() => history.back()}>
		<ArrowLeft class="size-4" aria-hidden="true" />Back
	</button>
	<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region aria-hidden="true"></span>
	<h1 class="signage text-base">Settings</h1>
</TitleBar>

<main class="min-h-0 flex-1 overflow-y-auto">
	<div class="mx-auto flex w-full max-w-2xl flex-col gap-10 px-6 py-10">
		<section class="flex flex-col gap-3" aria-labelledby="appearance">
			<h2 id="appearance" class="caps text-base-content/60">Appearance</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Theme</p>
						<p class="text-base-content/70">Every colour in the interface comes from it.</p>
					</div>
					<div class="flex items-center gap-3">
						<span class="flex items-center gap-1" aria-hidden="true">
							<span class="size-2.5 rounded-full bg-primary"></span>
							<span class="size-2.5 rounded-full bg-secondary"></span>
							<span class="size-2.5 rounded-full bg-accent"></span>
							<span class="size-2.5 rounded-full bg-warning"></span>
							<span class="size-2.5 rounded-full bg-info"></span>
							<span class="size-2.5 rounded-full bg-success"></span>
						</span>
						<select
							class="select w-44 select-sm"
							aria-label="Theme"
							value={appSettings.theme}
							onchange={(event) => appSettings.setTheme(event.currentTarget.value)}
						>
							<option value="">System theme</option>
							{#each themes as theme (theme)}
								<option value={theme}>{theme}</option>
							{/each}
						</select>
					</div>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Language</p>
						<p class="text-base-content/70">
							Translations are on the way; the choice is kept until they arrive.
						</p>
					</div>
					<select
						class="select w-44 select-sm"
						aria-label="Language"
						value={appSettings.locale}
						onchange={(event) => appSettings.setLocale(event.currentTarget.value)}
					>
						{#each languages as language (language.id)}
							<option value={language.id}>{language.name}</option>
						{/each}
					</select>
				</div>
			</div>
		</section>

		<section class="flex flex-col gap-3" aria-labelledby="storage">
			<h2 id="storage" class="caps text-base-content/60">Storage</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">App data</p>
						{#await appDataPath() then path}
							{#if path}
								<p class="truncate font-mono text-xs text-base-content/70" title={path}>{path}</p>
							{:else}
								<p class="text-base-content/70">Shown by the desktop app.</p>
							{/if}
						{/await}
					</div>
					<button
						type="button"
						class="btn btn-sm"
						disabled={!desktop}
						onclick={() => open(openDataDir)}>Open</button
					>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Repository caches</p>
						{#await appCachePath() then path}
							{#if path}
								<p class="truncate font-mono text-xs text-base-content/70" title={path}>{path}</p>
							{:else}
								<p class="text-base-content/70">Shown by the desktop app.</p>
							{/if}
						{/await}
					</div>
					<button
						type="button"
						class="btn btn-sm"
						disabled={!desktop}
						onclick={() => open(openCacheDir)}>Open</button
					>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Clear and rebuild caches</p>
						<p class="text-base-content/70">
							Drops every repository's cache and builds it again from its git directory.
						</p>
					</div>
					<button type="button" class="btn btn-sm" onclick={rebuild}>Rebuild</button>
				</div>
			</div>
			{#if storageNotice}
				<div role="status" class="alert alert-soft alert-info">{storageNotice}</div>
			{/if}
		</section>

		<section class="flex flex-col gap-3" aria-labelledby="about">
			<h2 id="about" class="caps text-base-content/60">About</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2 py-4">
					<div class="min-w-0">
						<p class="signage text-base">VOGT</p>
						<p class="text-base-content/70">Version &amp; Operations GUI Tool — a git client.</p>
					</div>
					{#await appVersion() then version}
						<p class="font-mono text-xs text-base-content/70">v{version}</p>
					{/await}
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Source and issues</p>
						<p class="text-base-content/70">Where VOGT is developed and reported on.</p>
					</div>
					<div class="flex gap-2">
						<a class="btn btn-sm" href={REPOSITORY} onclick={(event) => follow(event, REPOSITORY)}
							>GitHub</a
						>
						<a class="btn btn-sm" href={ISSUES} onclick={(event) => follow(event, ISSUES)}>Issues</a
						>
					</div>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Updates</p>
						<p class="text-base-content/70">Checks the release feed for a newer version.</p>
					</div>
					<button type="button" class="btn btn-sm" onclick={check}>Check for updates</button>
				</div>
			</div>
			{#if updateNotice}
				<div role="status" class="alert alert-soft alert-info">{updateNotice}</div>
			{/if}
		</section>
	</div>
</main>
