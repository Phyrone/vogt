<script lang="ts">
	import Keyboard from '@lucide/svelte/icons/keyboard';
	import Settings from '@lucide/svelte/icons/settings';
	import Archive from '@lucide/svelte/icons/archive';
	import Eye from '@lucide/svelte/icons/eye';
	import Tag from '@lucide/svelte/icons/tag';
	import type { ResolvedPathname } from '$app/types';
	import { lineStyle, type Line } from '#lib/repo/types.ts';

	interface Props {
		lines: Line[];
		headLine: string;
		ahead: number;
		stashes: string[];
		tags: string[];
		/** Ids of the lines the history is masked to; none means every line shows. */
		picked: string[];
		/** Where this repository's settings live. */
		settingsHref: ResolvedPathname;
	}

	let {
		lines,
		headLine,
		ahead,
		stashes,
		tags,
		picked = $bindable(),
		settingsHref
	}: Props = $props();

	const local = $derived(lines.filter((l) => !l.merged));

	function toggle(id: string) {
		picked = picked.includes(id) ? picked.filter((other) => other !== id) : [...picked, id];
	}
</script>

<nav
	class="flex w-14 shrink-0 flex-col border-r border-base-300 bg-base-200 lg:w-60"
	aria-label="Branches, stashes, tags and settings"
>
	<div class="min-h-0 flex-1 overflow-y-auto pb-4">
		<h2
			class="hidden items-baseline justify-between px-4 pt-4 pb-1.5 caps text-base-content/60 lg:flex"
		>
			<span>Branches</span><span class="font-mono text-[11px] font-normal tracking-normal"
				>{picked.length ? `${picked.length} of ${local.length}` : local.length}</span
			>
		</h2>
		<!-- the key to the map doubles as its mask: picking branches leaves only their lines lit -->
		<ol
			class="relative flex flex-col gap-0.5 px-2 pt-4 before:absolute before:top-7 before:bottom-5 before:left-[25px] before:w-1.5 before:rounded-full before:bg-base-300 lg:px-3 lg:pt-1 lg:before:top-4 lg:before:left-[29px]"
			aria-label="Show only these branches"
		>
			{#each local as line (line.id)}
				{@const on = picked.includes(line.id)}
				<li>
					<button
						type="button"
						class={[
							'relative flex w-full cursor-pointer items-center gap-3 rounded-field px-1 py-1.5 text-left motion-safe:transition-opacity',
							on ? 'bg-(--line)/15' : 'hover:bg-base-content/5',
							picked.length > 0 && !on && 'opacity-50 hover:opacity-100 focus-visible:opacity-100'
						]}
						style={lineStyle(line)}
						aria-pressed={on}
						title={on
							? `Stop showing only ${line.name}`
							: picked.length
								? `Also show ${line.name}`
								: `Show only ${line.name}`}
						onclick={() => toggle(line.id)}
					>
						<span
							class={[
								'grid size-8 shrink-0 place-items-center rounded-[7px] bg-(--line) signage text-lg leading-none text-(--line-content)',
								line.id === headLine &&
									'ring-2 ring-base-content ring-offset-2 ring-offset-base-200'
							]}
							aria-hidden="true">{line.badge}</span
						>
						<span class="hidden min-w-0 flex-1 lg:block">
							<span class="block truncate signage text-[15px] leading-tight font-semibold"
								>{line.name}</span
							>
							<span class="block truncate text-xs text-base-content/65"
								>{#if line.id === headLine}checked out{ahead > 0
										? ` · ${ahead} ahead`
										: ''}{:else if line.remote}on origin{:else}local only{/if}</span
							>
						</span>
						{#if on}<Eye class="mr-1 hidden size-4 shrink-0 lg:block" aria-hidden="true" />{/if}
						<span class="sr-only lg:hidden">{line.name}</span>
					</button>
				</li>
			{/each}
		</ol>
		<h2
			class="hidden items-baseline justify-between px-4 pt-6 pb-1.5 caps text-base-content/60 lg:flex"
		>
			<span>Stashes</span><span class="font-mono text-[11px] font-normal tracking-normal"
				>{stashes.length}</span
			>
		</h2>
		<ul class="menu hidden w-full gap-px menu-sm px-2 py-0 lg:flex">
			<!-- messages repeat; a stash is named by its place, stash@{i} -->
			{#each stashes as stash, i (i)}
				<li>
					<button type="button" class="gap-2.5 px-2"
						><Archive class="mx-[3px] size-4 shrink-0 opacity-60" aria-hidden="true" /><span
							class="truncate">{stash}</span
						></button
					>
				</li>
			{/each}
		</ul>
		<h2
			class="hidden items-baseline justify-between px-4 pt-6 pb-1.5 caps text-base-content/60 lg:flex"
		>
			<span>Tags</span><span class="font-mono text-[11px] font-normal tracking-normal"
				>{tags.length}</span
			>
		</h2>
		<div class="hidden flex-wrap gap-1.5 px-4 lg:flex">
			{#each tags as tag (tag)}
				<button type="button" class="badge cursor-pointer gap-1 badge-outline signage"
					><Tag class="size-3" aria-hidden="true" />{tag}</button
				>
			{/each}
		</div>
	</div>
	<div class="flex shrink-0 flex-col items-center gap-1 border-t border-base-300 p-2 lg:flex-row">
		<a
			class="btn btn-ghost btn-sm max-lg:btn-square lg:flex-1 lg:justify-start"
			href={settingsHref}
			title="Settings"
		>
			<Settings class="size-4 opacity-70" aria-hidden="true" /><span class="hidden lg:inline"
				>Settings</span
			><span class="sr-only lg:hidden">Settings</span>
		</a>
		<button
			type="button"
			class="btn btn-ghost btn-sm max-lg:btn-square lg:flex-1 lg:justify-start"
			title="Keyboard shortcuts"
		>
			<Keyboard class="size-4 opacity-70" aria-hidden="true" /><span class="hidden lg:inline"
				>Shortcuts</span
			><span class="sr-only lg:hidden">Keyboard shortcuts</span>
		</button>
	</div>
</nav>
