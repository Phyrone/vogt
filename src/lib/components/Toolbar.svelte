<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import GitBranchPlus from '@lucide/svelte/icons/git-branch-plus';
	import Redo2 from '@lucide/svelte/icons/redo-2';
	import Search from '@lucide/svelte/icons/search';
	import Undo2 from '@lucide/svelte/icons/undo-2';
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
	<h1>
		<button type="button" class="btn font-semibold btn-sm">
			<FolderGit2 class="size-4" aria-hidden="true" />{name}<ChevronDown
				class="size-3.5 opacity-60"
				aria-hidden="true"
			/>
		</button>
	</h1>
	<button type="button" class="btn font-semibold btn-sm">
		<LineBadge {line} size="sm" />
		<span class="max-w-40 truncate">{line.name}</span>
		<ChevronDown class="size-3.5 opacity-60" aria-hidden="true" />
	</button>
	<div class="hidden items-center gap-1 lg:flex" data-tauri-drag-region>
		<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-2xl:btn-square"
			title="Undo"
			aria-label="Undo"
		>
			<Undo2 class="size-4" aria-hidden="true" /><span class="hidden 2xl:inline" aria-hidden="true"
				>Undo</span
			>
		</button>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-2xl:btn-square"
			title="Redo"
			aria-label="Redo"
		>
			<Redo2 class="size-4" aria-hidden="true" /><span class="hidden 2xl:inline" aria-hidden="true"
				>Redo</span
			>
		</button>
	</div>
	<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
	<button type="button" class="btn btn-ghost btn-sm"
		><ArrowDownToLine class="size-4" aria-hidden="true" />Pull</button
	>
	<button type="button" class={['btn btn-sm', ahead > 0 ? 'btn-neutral' : 'btn-ghost']}>
		<ArrowUpFromLine class="size-4" aria-hidden="true" />Push{ahead > 0 ? ` ${ahead}` : ''}
	</button>
	<div class="hidden items-center gap-1 lg:flex" data-tauri-drag-region>
		<span class="mx-1 h-6 w-px bg-base-300" data-tauri-drag-region></span>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-2xl:btn-square"
			title="Branch"
			aria-label="Branch"
		>
			<GitBranchPlus class="size-4" aria-hidden="true" /><span
				class="hidden 2xl:inline"
				aria-hidden="true">Branch</span
			>
		</button>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-2xl:btn-square"
			title="Stash"
			aria-label="Stash"
		>
			<Archive class="size-4" aria-hidden="true" /><span
				class="hidden 2xl:inline"
				aria-hidden="true">Stash</span
			>
		</button>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-2xl:btn-square"
			title="Pop"
			aria-label="Pop"
		>
			<ArchiveRestore class="size-4" aria-hidden="true" /><span
				class="hidden 2xl:inline"
				aria-hidden="true">Pop</span
			>
		</button>
	</div>
	<label class="input ml-auto w-full max-w-90 min-w-32 input-sm">
		<Search class="size-4 opacity-60" aria-hidden="true" />
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
