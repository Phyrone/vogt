<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import { getLocale, locales, setLocale, type Locale } from '#lib/paraglide/runtime.js';
	import { m } from '#lib/paraglide/messages.js';
	import TitleBar from '#lib/components/TitleBar.svelte';
	import { SYSTEM, THEMES, theme, themeChoice } from '#lib/settings/app.svelte.ts';
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

	/** What to call a language in that language: English, Deutsch. */
	function languageName(locale: Locale): string {
		return new Intl.DisplayNames([locale], { type: 'language' }).of(locale) ?? locale;
	}

	let storageNotice = $state('');
	let updateNotice = $state('');
	/** The newer version the last check found. */
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
			available = await checkForUpdates();
			if (!available) updateNotice = m['settings.up_to_date']();
		} catch (reason) {
			updateNotice = String(reason);
		}
	}

	async function install(): Promise<void> {
		updateNotice = '';
		installing = true;
		try {
			// the app restarts into the new version, so this only returns when nothing was installed
			await installUpdate();
		} catch (reason) {
			updateNotice = String(reason);
		}
		installing = false;
	}

	function follow(event: MouseEvent, url: string): void {
		event.preventDefault();
		void openExternal(url);
	}
</script>

<TitleBar>
	<button type="button" class="btn btn-ghost btn-sm" onclick={() => history.back()}>
		<ArrowLeft class="size-4" aria-hidden="true" />{m['common.back']()}
	</button>
	<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region aria-hidden="true"></span>
	<h1 class="signage text-base">{m['common.settings']()}</h1>
</TitleBar>

<main class="min-h-0 flex-1 overflow-y-auto">
	<div class="mx-auto flex w-full max-w-2xl flex-col gap-10 px-6 py-10">
		<section class="flex flex-col gap-3" aria-labelledby="appearance">
			<h2 id="appearance" class="caps text-base-content/60">{m['settings.appearance']()}</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.theme']()}</p>
						<p class="text-base-content/70">{m['settings.theme_description']()}</p>
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
							aria-label={m['settings.theme']()}
							value={theme.current}
							onchange={(event) => (theme.current = themeChoice.parse(event.currentTarget.value))}
						>
							<option value={SYSTEM}>{m['settings.system_theme']()}</option>
							{#each THEMES as name (name)}
								<option value={name}>{name}</option>
							{/each}
						</select>
					</div>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.language']()}</p>
						<p class="text-base-content/70">{m['settings.language_description']()}</p>
					</div>
					<select
						class="select w-44 select-sm"
						aria-label={m['settings.language']()}
						value={getLocale()}
						onchange={(event) => void setLocale(event.currentTarget.value as Locale)}
					>
						{#each locales as tag (tag)}
							<option value={tag}>{languageName(tag)}</option>
						{/each}
					</select>
				</div>
			</div>
		</section>

		<section class="flex flex-col gap-3" aria-labelledby="storage">
			<h2 id="storage" class="caps text-base-content/60">{m['settings.storage']()}</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.app_data']()}</p>
						{#await appDataPath() then path}
							{#if path}
								<p class="truncate font-mono text-xs text-base-content/70" title={path}>{path}</p>
							{:else}
								<p class="text-base-content/70">{m['settings.shown_by_desktop']()}</p>
							{/if}
						{/await}
					</div>
					<button
						type="button"
						class="btn btn-sm"
						disabled={!desktop}
						onclick={() => open(openDataDir)}>{m['common.open']()}</button
					>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.repository_caches']()}</p>
						{#await appCachePath() then path}
							{#if path}
								<p class="truncate font-mono text-xs text-base-content/70" title={path}>{path}</p>
							{:else}
								<p class="text-base-content/70">{m['settings.shown_by_desktop']()}</p>
							{/if}
						{/await}
					</div>
					<button
						type="button"
						class="btn btn-sm"
						disabled={!desktop}
						onclick={() => open(openCacheDir)}>{m['common.open']()}</button
					>
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.clear_caches']()}</p>
						<p class="text-base-content/70">{m['settings.clear_caches_description']()}</p>
					</div>
					<button type="button" class="btn btn-sm" onclick={rebuild}>{m['common.rebuild']()}</button
					>
				</div>
			</div>
			{#if storageNotice}
				<div role="status" class="alert alert-soft alert-info">{storageNotice}</div>
			{/if}
		</section>

		<section class="flex flex-col gap-3" aria-labelledby="about">
			<h2 id="about" class="caps text-base-content/60">{m['settings.about']()}</h2>
			<div class="flex flex-col divide-y divide-base-300 border-y border-base-300">
				<div class="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2 py-4">
					<div class="min-w-0">
						<p class="signage text-base">VOGT</p>
						<p class="text-base-content/70">{m['settings.tagline']()}</p>
					</div>
					{#await appVersion() then version}
						<p class="font-mono text-xs text-base-content/70">v{version}</p>
					{/await}
				</div>
				<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 py-4">
					<div class="min-w-0">
						<p class="font-semibold">{m['settings.source_issues']()}</p>
						<p class="text-base-content/70">{m['settings.source_issues_description']()}</p>
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
						<p class="font-semibold">{m['settings.updates']()}</p>
						<p class="text-base-content/70">{m['settings.updates_description']()}</p>
					</div>
					<button type="button" class="btn btn-sm" onclick={check}
						>{m['settings.check_updates']()}</button
					>
				</div>
			</div>
			{#if available}
				<div role="status" class="alert alert-soft alert-info">
					<span>{m['settings.update_available']({ version: available })}</span>
					<button type="button" class="btn btn-sm" disabled={installing} onclick={install}>
						{installing ? m['settings.installing']() : m['settings.install_restart']()}
					</button>
				</div>
			{/if}
			{#if updateNotice}
				<div role="status" class="alert alert-soft alert-info">{updateNotice}</div>
			{/if}
		</section>
	</div>
</main>
