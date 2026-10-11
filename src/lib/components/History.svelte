<script lang="ts">
	import { graphWidth, layoutGraph, PITCH } from '#lib/graph/layout.ts';
	import { lineStyle, type Commit, type Line } from '#lib/repo/types.ts';
	import CommitGraph from './CommitGraph.svelte';

	interface Props {
		/** Newest first; uncommitted work, when there is any, is the first row. */
		rows: Commit[];
		lines: Record<string, Line>;
		trunk: string;
		head: string;
		selected: string;
		focusLine: string | null;
	}

	let { rows, lines, trunk, head, selected = $bindable(), focusLine }: Props = $props();

	const layout = $derived(layoutGraph(rows, trunk));
	const graph = $derived(graphWidth(layout.lanes));

	function onkeydown(event: KeyboardEvent & { currentTarget: HTMLElement }) {
		const step = { ArrowDown: 1, j: 1, ArrowUp: -1, k: -1 }[event.key];
		if (!step) return;
		const next = rows[rows.findIndex((c) => c.hash === selected) + step];
		if (!next) return;
		event.preventDefault();
		selected = next.hash;
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
	<div class="min-h-0 flex-1 overflow-y-auto">
		<div class="relative">
			<CommitGraph {layout} {lines} {head} {focusLine} />
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<ol {onkeydown}>
				{#each rows as commit (commit.hash)}
					{@const line = lines[commit.line]}
					<li>
						<button
							type="button"
							class={[
								'flex w-full cursor-pointer items-center gap-3 pr-4 text-left -outline-offset-2 motion-safe:transition-opacity',
								selected === commit.hash
									? 'bg-(--line)/12 font-semibold'
									: 'hover:bg-base-content/5',
								focusLine !== null && focusLine !== commit.line && 'opacity-45'
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
								>{commit.pending ? '' : commit.hash}</span
							>
							<span
								class="hidden w-20 shrink-0 text-right font-normal text-base-content/70 @lg:block"
								>{commit.when}</span
							>
						</button>
					</li>
				{/each}
			</ol>
		</div>
	</div>
</section>
