<script lang="ts">
	import '@fontsource/barlow/400.css';
	import '@fontsource/barlow/500.css';
	import '@fontsource/barlow/600.css';
	import '@fontsource/barlow-semi-condensed/600.css';
	import '@fontsource/barlow-semi-condensed/700.css';
	import '@fontsource-variable/jetbrains-mono';
	import '../app.css';
	import Theme from '#lib/components/Theme.svelte';
	import { locale } from '#lib/settings/app.svelte.ts';

	const { children } = $props();

	// the language lives on the document, so every page follows it
	$effect(() => {
		document.documentElement.lang = locale.current;
	});
</script>

<!-- every page fills the window: a title bar of its own, then its content -->
<div class="flex h-dvh flex-col bg-base-100 text-sm">
	<Theme />
	{@render children()}
</div>
