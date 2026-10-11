<script lang="ts">
	import { createVirtualizer } from '@tanstack/svelte-virtual';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { graphWidth, MAX_LANES, PITCH } from '#lib/graph/layout.ts';
	import {
		lineStyle,
		type Commit,
		type HistoryEdge,
		type HistoryRow,
		type HistoryWindow,
		type Line
	} from '#lib/repo/types.ts';
	import X from '@lucide/svelte/icons/x';
	import CommitGraph from './CommitGraph.svelte';
	import LineBadge from './LineBadge.svelte';

	interface Props {
		/** Where the rows come from. They are asked for as they scroll into view. */
		source: {
			rows(from: number, to: number, only: string[]): Promise<HistoryWindow>;
			/** Changes when the history did, so that it is asked for again. */
			version: number;
		};
		lines: Record<string, Line>;
		trunk: string;
		head: string;
		/** The selected commit; null selects uncommitted work, when there is any. */
		selected: Commit | null;
		/** Ids of the lines picked in the sidebar. While there are any, the rest is dimmed or hidden. */
		picked: string[];
	}

	let {
		source,
		lines,
		trunk,
		head,
		selected = $bindable(),
		picked = $bindable()
	}: Props = $props();

	/** Rows are asked for this many at a time. */
	const CHUNK = 100;

	/** Leave the rest out instead of dimming it. */
	let hide = $state(false);
	const only = $derived(hide ? picked : []);

	let total = $state(0);
	let lanes = $state(1);
	let failure = $state('');
	/** The windows that arrived, by the number of their chunk. */
	const chunks = new SvelteMap<number, HistoryWindow>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping nothing is drawn from
	const requested = new Set<number>();
	/** Goes up when everything is asked for again; answers to earlier questions are dropped. */
	let epoch = $state(0);

	function load(chunk: number) {
		if (requested.has(chunk)) return;
		requested.add(chunk);
		const asked = epoch;
		source.rows(chunk * CHUNK, (chunk + 1) * CHUNK, only).then(
			(window) => {
				if (asked !== epoch) return;
				chunks.set(chunk, window);
				total = window.total;
				lanes = window.lanes;
				// with no working changes to show, start on the checked-out commit
				if (selected === null && !window.rows[0]?.pending) {
					selected = window.rows.find((row) => row.hash === head) ?? null;
				}
			},
			(reason) => {
				if (asked === epoch) failure = String(reason);
			}
		);
	}

	// the history changed or is masked differently: every row is asked for again
	$effect(() => {
		void source.version;
		void only.join('\n');
		untrack(() => {
			epoch++;
			requested.clear();
			chunks.clear();
			failure = '';
			load(0);
		});
	});

	// only the rows in view are in the document, so a history can be any length
	let scroller: HTMLDivElement;
	const virtualizer = createVirtualizer<HTMLDivElement, HTMLLIElement>({
		count: 0,
		getScrollElement: () => scroller,
		estimateSize: () => PITCH,
		overscan: 12
	});
	$effect(() => {
		const count = total;
		untrack(() => $virtualizer.setOptions({ count }));
	});
	const items = $derived($virtualizer.getVirtualItems());
	const first = $derived(items[0]?.index ?? 0);
	const last = $derived(items.at(-1)?.index ?? -1);

	$effect(() => {
		void epoch;
		const [from, to] = [Math.floor(first / CHUNK), Math.floor(last / CHUNK)];
		untrack(() => {
			for (let chunk = from; chunk <= to; chunk++) load(chunk);
		});
	});

	const rowAt = (index: number) => chunks.get(Math.floor(index / CHUNK))?.rows[index % CHUNK];
	const isSelected = (commit: Commit) =>
		selected ? selected.hash === commit.hash : Boolean(commit.pending);
	const select = (commit: Commit) => (selected = commit.pending ? null : commit);

	/** What the graph draws: the stations in view and every edge that crosses them. */
	const shown = $derived.by(() => {
		const nodes: HistoryRow[] = [];
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local to this computation
		const edges = new Map<string, HistoryEdge>();
		for (let chunk = Math.floor(first / CHUNK); chunk <= Math.floor(last / CHUNK); chunk++) {
			const window = chunks.get(chunk);
			if (!window) continue;
			for (const row of window.rows) if (row.row >= first && row.row <= last) nodes.push(row);
			// an edge that crosses two chunks arrives with both
			for (const edge of window.edges) {
				if (edge.from <= last && edge.to >= first) edges.set(`${edge.from}-${edge.to}`, edge);
			}
		}
		// the trunk is drawn first so that lines joining it sit on top
		const drawn = [...edges.values()].sort(
			(a, b) => Number(a.line !== trunk) - Number(b.line !== trunk)
		);
		return { nodes, edges: drawn };
	});

	// never narrower than two lanes: the column's heading needs the room
	const graph = $derived(graphWidth(Math.min(Math.max(lanes, 2), MAX_LANES)));

	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLElement }) {
		const step = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 }[event.key];
		const current = shown.nodes.find(isSelected);
		// the neighbour of a row in view is within the overscan, so it has arrived and is in the document
		const next = step && current && rowAt(current.row + step);
		if (!next) return;
		event.preventDefault();
		select(next);
		$virtualizer.scrollToIndex(next.row);
		event.currentTarget.querySelector<HTMLElement>(`[data-hash="${next.hash}"]`)?.focus();
	}
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
	{#if failure}
		<p role="alert" class="p-6 text-error">The history could not be read: {failure}</p>
	{:else if !total && chunks.has(0)}
		<p class="p-6 text-base-content/70">
			{only.length ? 'No commits were made on these branches themselves.' : 'No commits yet.'}
		</p>
	{/if}
	<div class="min-h-0 flex-1 overflow-y-auto" bind:this={scroller}>
		<div class="relative" style:height="{total * PITCH}px">
			<CommitGraph
				nodes={shown.nodes}
				edges={shown.edges}
				width={graph}
				height={total * PITCH}
				{lines}
				{head}
				{picked}
			/>
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<ol {onkeydown}>
				{#each items as item (item.index)}
					{@const commit = rowAt(item.index)}
					<li
						class="absolute top-0 left-0 w-full"
						style:transform="translateY({item.start}px)"
						aria-setsize={total}
						aria-posinset={item.index + 1}
					>
						{#if commit}
							{@const line = lines[commit.line]}
							<button
								type="button"
								class={[
									'flex w-full cursor-pointer items-center gap-3 pr-4 text-left -outline-offset-2 motion-safe:transition-opacity',
									isSelected(commit) ? 'bg-(--line)/12 font-semibold' : 'hover:bg-base-content/5',
									picked.length > 0 && !picked.includes(commit.line) && 'opacity-45'
								]}
								style="{lineStyle(line)}; height: {PITCH}px"
								data-hash={commit.hash}
								aria-pressed={isSelected(commit)}
								onclick={() => select(commit)}
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
						{:else}
							<!-- the row is on its way -->
							<div class="flex items-center gap-3 pr-4" style:height="{PITCH}px" aria-hidden="true">
								<span class="shrink-0" style:width="{graph}px"></span>
								<span class="h-2.5 w-2/5 rounded-full bg-base-content/10"></span>
							</div>
						{/if}
					</li>
				{/each}
			</ol>
		</div>
	</div>
</section>
