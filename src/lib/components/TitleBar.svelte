<script lang="ts">
	import type { Snippet } from 'svelte';
	import { trackWindow, windowChrome } from '#lib/window.svelte.ts';
	import SettingsMenu from './SettingsMenu.svelte';
	import WindowControls from './WindowControls.svelte';

	interface Props {
		/** Leading controls. */
		children?: Snippet;
		/** Controls grouped at the trailing end, before the settings menu and the window controls. */
		tools?: Snippet;
	}

	let { children, tools }: Props = $props();

	const appWindow = trackWindow();
</script>

<!-- the only window chrome there is: its empty areas drag the window -->
<header
	class={[
		'flex h-14 shrink-0 items-center gap-1 border-b border-base-300 bg-base-200 px-3 whitespace-nowrap',
		windowChrome === 'mac' && !appWindow.fullscreen && 'pl-21'
	]}
	data-tauri-drag-region
>
	{@render children?.()}
	<div class="ml-auto flex items-center gap-2" data-tauri-drag-region>
		{@render tools?.()}
		<SettingsMenu />
		{#if windowChrome === 'custom'}
			<WindowControls maximized={appWindow.maximized} />
		{/if}
	</div>
</header>
