<script lang="ts">
	import '@fontsource/barlow/400.css';
	import '@fontsource/barlow/500.css';
	import '@fontsource/barlow/600.css';
	import '@fontsource/barlow-semi-condensed/600.css';
	import '@fontsource/barlow-semi-condensed/700.css';
	import '@fontsource-variable/jetbrains-mono';
	import '../app.css';
	import { installMenu } from '#lib/menu.ts';
	import { appSettings } from '#lib/settings/app.svelte.ts';

	const { children } = $props();

	// the theme and the language live on the document, so every page follows them
	$effect(() => appSettings.apply());
	// the native window menu, once per app run
	$effect(() => {
		void installMenu().catch((reason) => console.error('vogt: no window menu', reason));
	});
</script>

<!-- every page fills the window: a title bar of its own, then its content -->
<div class="flex h-dvh flex-col bg-base-100 text-sm">
	{@render children()}
</div>
