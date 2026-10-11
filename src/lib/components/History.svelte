<script lang="ts">
	import { createVirtualizer } from '@tanstack/svelte-virtual';
	import { untrack } from 'svelte';
	import { graphWidth, layoutGraph, PITCH } from '#lib/graph/layout.ts';
	import { lineStyle, type Commit, type Line } from '#lib/repo/types.ts';
	import X from '@lucide/svelte/icons/x';
	import CommitGraph from './CommitGraph.svelte';
	import LineBadge from './LineBadge.svelte';

	interface Props {
		/** Newest first; uncommitted work, when there is any, is the first row. */
		rows: Commit[];
		lines: Record<string, Line>;
		trunk: string;
		head: string;
		selected: string;
		/** Ids of the lines picked in the sidebar. While there are any, the rest is dimmed or hidden. */
		picked: string[];
	}

	let {
		rows: all,
		lines,
		trunk,
		head,
		selected = $bindable(),
		picked = $bindable()
	}: Props = $props();

	/** Leave the rest out instead of dimming it. */
	let hide = $state(false);
	const rows = $derived(
		hide && picked.length ? all.filter((commit) => picked.includes(commit.line)) : all
	);

	const layout = $derived(layoutGraph(rows, trunk));
	// never narrower than two lanes: the column's heading needs the room
	const graph = $derived(graphWidth(Math.max(layout.lanes, 2)));

	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLElement }) {
		const step = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 }[event.key];
		if (!step) return;
		const next = rows[rows.findIndex((c) => c.hash === selected) + step];
		if (!next) return;
		event.preventDefault();
		selected = next.hash;
		// the neighbour of a rendered row is within the overscan, so it is in the document already
		$virtualizer.scrollToIndex(rows.indexOf(next));
		event.currentTarget.querySelector<HTMLElement>(`[data-hash="${next.hash}"]`)?.focus();
	}

	// only the rows in view are in the document, so a history can be any length
	let scroller: HTMLDivElement;
	const virtualizer = createVirtualizer<HTMLDivElement, HTMLLIElement>({
		count: 0,
		getScrollElement: () => scroller,
		estimateSize: () => PITCH,
		overscan: 12
	});
	$effect(() => {
		const count = rows.length;
		untrack(() => $virtualizer.setOptions({ count }));
	});
	const items = $derived($virtualizer.getVirtualItems());
</script>

<section class="@container flex min-w-0 flex-1 flex-col" aria-label="History">
	<div
		class="flex h-8 shrink-0 items-center gap-3 border-b border-base-300 pr-4 caps text-base-content/70"
	>
		<span class="shrink-0 pl-4" style:width="{graph}px">Graph</span>
		<span class="min-w-0 flex-1">Commit</span>
		<span class="hidden w-20 shrink-0 @2xl:block">Author</span>
		<span class="hidden w-16 shrink-0 @2xl:block">Hash</span>
		<span class="hidden w-20 shrink-0 text-right @lg:block">When</span>
	</div>
	{#if picked.length}
		<div
			class="flex h-9 shrink-0 items-center gap-3 border-b border-base-300 bg-base-200 pr-2 pl-4"
			role="group"
			aria-label="Branches shown"
		>
			<span class="shrink-0 caps text-base-content/70">Only</span>
			<span class="flex min-w-0 flex-1 items-center gap-3 overflow-hidden">
				{#each picked as id (id)}
					{@const line = lines[id]}
					{#if line}
						<span class="flex min-w-0 items-center gap-1.5">
							<LineBadge {line} size="sm" /><span class="truncate font-semibold">{line.name}</span>
						</span>
					{/if}
				{/each}
			</span>
			<div class="join shrink-0">
				<button
					type="button"
					class={['btn join-item btn-xs', !hide && 'btn-neutral']}
					aria-pressed={!hide}
					onclick={() => (hide = false)}>Dim the rest</button
				>
				<button
					type="button"
					class={['btn join-item btn-xs', hide && 'btn-neutral']}
					aria-pressed={hide}
					onclick={() => (hide = true)}>Hide the rest</button
				>
			</div>
			<button type="button" class="btn shrink-0 btn-ghost btn-xs" onclick={() => (picked = [])}
				><X class="size-3.5" aria-hidden="true" />Show all</button
			>
		</div>
	{/if}
	{#if !rows.length && all.length}
		<p class="p-6 text-base-content/70">No commits were made on these branches themselves.</p>
	{/if}
	<div class="min-h-0 flex-1 overflow-y-auto" bind:this={scroller}>
		<div class="relative" style:height="{rows.length * PITCH}px">
			<CommitGraph
				{layout}
				rows={[items[0]?.index ?? 0, items.at(-1)?.index ?? -1]}
				{lines}
				{head}
				{picked}
			/>
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<ol {onkeydown}>
				{#each items as item (item.index)}
					{@const commit = rows[item.index]}
					{#if commit}
						{@const line = lines[commit.line]}
						<li
							class="absolute top-0 left-0 w-full"
							style:transform="translateY({item.start}px)"
							aria-setsize={rows.length}
							aria-posinset={item.index + 1}
						>
							<button
								type="button"
								class={[
									'flex w-full cursor-pointer items-center gap-3 pr-4 text-left -outline-offset-2 motion-safe:transition-opacity',
									selected === commit.hash
										? 'bg-(--line)/12 font-semibold'
										: 'hover:bg-base-content/5',
									picked.length > 0 && !picked.includes(commit.line) && 'opacity-45'
								]}
								style="{lineStyle(line)}; height: {PITCH}px"
								data-hash={commit.hash}
								aria-pressed={selected === commit.hash}
								onclick={() => (selected = commit.hash)}
							>
								<span class="shrink-0" style:width="{graph}px"></span>
								<span class="flex min-w-0 flex-1 items-center gap-2">
									{#if commit.hash === head}
										<span class="badge shrink-0 signage badge-sm tracking-wider badge-neutral"
											>HERE</span
										>
									{/if}
									{#each commit.refs ?? [] as ref (ref.label)}
										<span class="badge shrink-0 badge-outline signage badge-sm">{ref.label}</span>
									{/each}
									<span class="truncate">{commit.subject}</span>
									<span class="sr-only">on {line.name}</span>
								</span>
								<span
									class="hidden w-20 shrink-0 truncate font-normal text-base-content/70 @2xl:block"
									>{commit.author}</span
								>
								<span
									class="hidden w-16 shrink-0 font-mono text-xs font-normal text-base-content/70 @2xl:block"
									>{commit.pending ? '' : commit.hash.slice(0, 7)}</span
								>
								<span
									class="hidden w-20 shrink-0 text-right font-normal text-base-content/70 @lg:block"
									>{commit.when}</span
								>
							</button>
						</li>
					{/if}
				{/each}
			</ol>
		</div>
	</div>
</section>
