<script lang="ts">
	import { SYSTEM, theme } from '#lib/settings/app.svelte.ts';

	// the theme lives on the document, so every page follows it
	$effect(() => {
		const active = theme.current;
		if (active !== SYSTEM) document.documentElement.dataset.theme = active;
		else delete document.documentElement.dataset.theme;
	});
</script>
