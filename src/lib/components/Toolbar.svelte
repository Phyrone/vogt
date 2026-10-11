<script lang="ts">
	import type { Line } from '#lib/repo/types.ts';
	import LineBadge from './LineBadge.svelte';
	import { trackWindow, windowChrome } from '#lib/window.svelte.ts';
	import ThemeSelect from './ThemeSelect.svelte';
	import WindowControls from './WindowControls.svelte';

	interface Props {
		name: string;
		line: Line;
		ahead: number;
	}

	let { name, line, ahead }: Props = $props();

	const appWindow = trackWindow();

	let command: HTMLInputElement;
	const modifier = navigator.platform.startsWith('Mac') ? '⌘' : 'Ctrl';

	function onkeydown(event: KeyboardEvent) {
		if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
			event.preventDefault();
			command.focus();
		}
	}
</script>

<svelte:window {onkeydown} />

<!-- the toolbar doubles as the title bar: its empty areas drag the window -->
<header
	class={[
		'flex h-14 shrink-0 items-center gap-2 border-b border-base-300 bg-base-200 px-3 whitespace-nowrap',
		// leave room for the traffic lights macOS overlays; fullscreen hides them
		windowChrome === 'mac' && !appWindow.fullscreen && 'pl-21'
	]}
	data-tauri-drag-region
>
	<h1><button type="button" class="btn font-semibold btn-sm">{name}</button></h1>
	<button type="button" class="btn font-semibold btn-sm">
		<LineBadge {line} size="sm" />
		<span class="max-w-40 truncate">{line.name}</span>
	</button>
	<div class="hidden items-center gap-1 lg:flex" data-tauri-drag-region>
		<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
		<button type="button" class="btn btn-ghost btn-sm">Undo</button>
		<button type="button" class="btn btn-ghost btn-sm">Redo</button>
	</div>
	<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
	<button type="button" class="btn btn-ghost btn-sm">Pull</button>
	<button type="button" class={['btn btn-sm', ahead > 0 ? 'btn-neutral' : 'btn-ghost']}>
		Push{ahead > 0 ? ` ${ahead}` : ''}
	</button>
	<div class="hidden items-center gap-1 xl:flex" data-tauri-drag-region>
		<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
		<button type="button" class="btn btn-ghost btn-sm">Branch</button>
		<button type="button" class="btn btn-ghost btn-sm">Stash</button>
		<button type="button" class="btn btn-ghost btn-sm">Pop</button>
	</div>
	<label class="input ml-auto w-full max-w-90 min-w-32 input-sm">
		<input
			bind:this={command}
			type="search"
			placeholder="Go to a commit, branch or command"
			aria-label="Go to a commit, branch or command"
		/>
		<kbd class="kbd kbd-sm">{modifier} K</kbd>
	</label>
	<ThemeSelect />
	{#if windowChrome === 'custom'}
		<WindowControls maximized={appWindow.maximized} />
	{/if}
</header>
