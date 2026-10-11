<script lang="ts">
	import { lineStyle, type Line } from '#lib/repo/types.ts';
	import LineBadge from './LineBadge.svelte';

	interface Props {
		lines: Line[];
		headLine: string;
		ahead: number;
		stashes: string[];
		tags: string[];
		focusLine: string | null;
	}

	let { lines, headLine, ahead, stashes, tags, focusLine = $bindable() }: Props = $props();

	const local = $derived(lines.filter((l) => !l.merged));
	const remote = $derived(local.filter((l) => l.remote));
</script>

<nav
	class="w-14 shrink-0 overflow-y-auto border-r border-base-300 bg-base-200 pb-4 lg:w-60"
	aria-label="Branches, stashes and tags"
>
	<h2 class="hidden px-4 pt-4 pb-1 caps text-base-content/70 lg:block">Local</h2>
	<ul class="menu w-full gap-0.5 px-2 pt-3 lg:pt-0">
		{#each local as line (line.id)}
			<li>
				<button
					type="button"
					class={[
						'justify-center gap-2.5 px-2 lg:justify-start',
						focusLine === line.id && 'bg-(--line)/15 font-semibold'
					]}
					style={lineStyle(line)}
					aria-pressed={focusLine === line.id}
					title="Highlight {line.name} in the graph"
					onclick={() => (focusLine = focusLine === line.id ? null : line.id)}
				>
					<LineBadge {line} />
					<span class="hidden min-w-0 flex-1 truncate lg:block">{line.name}</span>
					<span class="sr-only lg:hidden">{line.name}</span>
					{#if line.id === headLine && ahead > 0}
						<span class="hidden font-mono text-[11px] font-normal lg:block">{ahead} ahead</span>
					{/if}
				</button>
			</li>
		{/each}
	</ul>
	<div class="hidden lg:block">
		<h2 class="px-4 pt-4 pb-1 caps text-base-content/70">Remote · origin</h2>
		<ul class="menu w-full gap-0.5 px-2 py-0">
			{#each remote as line (line.id)}
				<li>
					<button type="button" class="gap-2.5 px-2">
						<LineBadge {line} />
						<span class="min-w-0 flex-1 truncate">{line.name}</span>
					</button>
				</li>
			{/each}
		</ul>
		<h2 class="px-4 pt-4 pb-1 caps text-base-content/70">Stashes</h2>
		<ul class="menu w-full gap-0.5 px-2 py-0">
			{#each stashes as stash (stash)}
				<li>
					<button type="button" class="pl-[42px]"><span class="truncate">{stash}</span></button>
				</li>
			{/each}
		</ul>
		<h2 class="px-4 pt-4 pb-1 caps text-base-content/70">Tags</h2>
		<ul class="menu w-full gap-0.5 px-2 py-0">
			{#each tags as tag (tag)}
				<li><button type="button" class="pl-[42px]">{tag}</button></li>
			{/each}
		</ul>
	</div>
</nav>
