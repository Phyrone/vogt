<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import TitleBar from '#lib/components/TitleBar.svelte';
	import {
		CHANNELS,
		channel,
		channelChoice,
		LANGUAGES,
		locale,
		localeChoice,
		SYSTEM,
		THEMES,
		theme,
		themeChoice
	} from '#lib/settings/app.svelte.ts';
	import {
		appCachePath,
		appDataPath,
		appVersion,
		checkForUpdates,
		desktop,
		installUpdate,
		openCacheDir,
		openDataDir,
		openExternal,
		rebuildCaches
	} from '#lib/settings/system.ts';

	const REPOSITORY = 'https://github.com/Phyrone/vogt';
	const ISSUES = 'https://github.com/Phyrone/vogt/issues';

	let storageNotice = $state('');
	let updateNotice = $state('');
	/** The newer version the last check found on the chosen channel. */
	let available = $state<string | null>(null);
	let installing = $state(false);

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
		available = null;
		try {
			available = await checkForUpdates(channel.current);
			if (!available) updateNotice = 'VOGT is up to date.';
		} catch (reason) {
			updateNotice = String(reason);
		}
	}

	async function install(): Promise<void> {
		updateNotice = '';
		installing = true;
		try {
			// the app restarts into the new version, so this only returns when nothing was installed
			await installUpdate(channel.current);
		} catch (reason) {
			updateNotice = String(reason);
		}
		installing = false;
	}

	function switchChannel(value: string): void {
		channel.current = channelChoice.parse(value);
		// what the other channel offered says nothing about this one
		available = null;
		updateNotice = '';
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
							value={theme.current}
							onchange={(event) => (theme.current = themeChoice.parse(event.currentTarget.value))}
						>
							<option value={SYSTEM}>System theme</option>
							{#each THEMES as name (name)}
								<option value={name}>{name}</option>
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
						value={locale.current}
						onchange={(event) => (locale.current = localeChoice.parse(event.currentTarget.value))}
					>
						{#each LANGUAGES as language (language.id)}
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
						<p class="font-semibold">Update channel</p>
						<p class="text-base-content/70">
							Beta gets a release before it is declared stable, rough edges included.
						</p>
					</div>
					<select
						class="select w-44 select-sm"
						aria-label="Update channel"
						value={channel.current}
						onchange={(event) => switchChannel(event.currentTarget.value)}
					>
						{#each CHANNELS as option (option.id)}
							<option value={option.id}>{option.name}</option>
						{/each}
					</select>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">Updates</p>
						<p class="text-base-content/70">Checks the channel for a newer version.</p>
					</div>
					<button type="button" class="btn btn-sm" onclick={check}>Check for updates</button>
				</div>
			</div>
			{#if available}
				<div role="status" class="alert alert-soft alert-info">
					<span>VOGT {available} is available.</span>
					<button type="button" class="btn btn-sm" disabled={installing} onclick={install}>
						{installing ? 'Installing…' : 'Install and restart'}
					</button>
				</div>
			{/if}
			{#if updateNotice}
				<div role="status" class="alert alert-soft alert-info">{updateNotice}</div>
			{/if}
		</section>
	</div>
</main>
