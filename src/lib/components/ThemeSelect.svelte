<script lang="ts">
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
	const KEY = 'vogt:theme';

	function stored(): string {
		try {
			return localStorage.getItem(KEY) ?? '';
		} catch {
			return '';
		}
	}

	// '' follows the system: light, or dark when the system prefers it
	let theme = $state(stored());

	$effect(() => {
		if (theme) document.documentElement.dataset.theme = theme;
		else delete document.documentElement.dataset.theme;
		try {
			localStorage.setItem(KEY, theme);
		} catch {
			// storage is unavailable; the choice lasts for this session only
		}
	});
</script>

<select class="select w-32 shrink-0 select-sm" aria-label="Theme" bind:value={theme}>
	<option value="">System theme</option>
	{#each themes as name (name)}
		<option value={name}>{name}</option>
	{/each}
</select>
