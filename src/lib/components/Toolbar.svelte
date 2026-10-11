<script lang="ts">
	import Archive from '@lucide/svelte/icons/archive';
	import ArchiveRestore from '@lucide/svelte/icons/archive-restore';
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
	import GitBranchPlus from '@lucide/svelte/icons/git-branch-plus';
	import Redo2 from '@lucide/svelte/icons/redo-2';
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
</script>

<!-- the toolbar doubles as the title bar: its empty areas drag the window -->
<header
	class={[
		'flex h-14 shrink-0 items-center gap-1 border-b border-base-300 bg-base-200 px-3 whitespace-nowrap',
		windowChrome === 'mac' && !appWindow.fullscreen && 'pl-21'
	]}
	data-tauri-drag-region
>
	<h1>
		<button type="button" class="btn btn-ghost font-semibold btn-sm"
			><FolderGit2 class="size-4" aria-hidden="true" />{name}<ChevronDown
				class="size-3.5 opacity-60"
				aria-hidden="true"
			/></button
		>
	</h1>
	<span class="text-base-content/40" aria-hidden="true">/</span>
	<button type="button" class="btn btn-ghost font-semibold btn-sm"
		><LineBadge {line} size="sm" /><span class="max-w-40 truncate">{line.name}</span><ChevronDown
			class="size-3.5 opacity-60"
			aria-hidden="true"
		/></button
	>
	<span class="mx-2 h-6 w-px bg-base-300" data-tauri-drag-region></span>
	<button type="button" class="btn btn-ghost btn-sm"
		><ArrowDownToLine class="size-4" aria-hidden="true" />Pull</button
	>
	<button type="button" class={['btn btn-sm', ahead > 0 ? 'btn-neutral' : 'btn-ghost']}
		><ArrowUpFromLine class="size-4" aria-hidden="true" />Push{ahead > 0 ? ` ${ahead}` : ''}</button
	>
	<div class="ml-auto flex items-center gap-2" data-tauri-drag-region>
		<div class="hidden items-center lg:flex">
			<button type="button" class="btn btn-square btn-ghost btn-sm" title="Undo" aria-label="Undo"
				><Undo2 class="size-4" aria-hidden="true" /></button
			>
			<button type="button" class="btn btn-square btn-ghost btn-sm" title="Redo" aria-label="Redo"
				><Redo2 class="size-4" aria-hidden="true" /></button
			>
			<button
				type="button"
				class="btn btn-square btn-ghost btn-sm"
				title="Branch"
				aria-label="Branch"><GitBranchPlus class="size-4" aria-hidden="true" /></button
			>
			<button type="button" class="btn btn-square btn-ghost btn-sm" title="Stash" aria-label="Stash"
				><Archive class="size-4" aria-hidden="true" /></button
			>
			<button type="button" class="btn btn-square btn-ghost btn-sm" title="Pop" aria-label="Pop"
				><ArchiveRestore class="size-4" aria-hidden="true" /></button
			>
		</div>
		<ThemeSelect />
		{#if windowChrome === 'custom'}
			<WindowControls maximized={appWindow.maximized} />
		{/if}
	</div>
</header>
