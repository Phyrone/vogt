<script lang="ts">
	import { INSET, PITCH } from '#lib/graph/layout.ts';
	import type { HistoryEdge, HistoryRow, Line } from '#lib/repo/types.ts';

	interface Props {
		/** The stations to draw; the rest of a long history is left out. */
		nodes: HistoryRow[];
		/** The edges to draw, in the order they are stacked. */
		edges: HistoryEdge[];
		/** The size of the whole graph, of which only the part in view is drawn. */
		width: number;
		height: number;
		lines: Record<string, Line>;
		head: string;
		/** Ids of the lines picked in the sidebar; while there are any, every other line is dimmed. */
		picked: string[];
	}

	let { nodes, edges, width, height, lines, head, picked }: Props = $props();

	const x = (lane: number) => INSET + lane * PITCH;
	const y = (row: number) => PITCH / 2 + row * PITCH;
	const color = (line: string) => `var(--color-${lines[line]?.color ?? 'neutral'})`;
	const path = (points: [number, number][]) =>
		points.map(([lane, row], i) => `${i ? 'L' : 'M'}${x(lane)} ${y(row)}`).join('');
	const opacity = (line: string) => (picked.length && !picked.includes(line) ? 0.25 : 1);
</script>

<svg
	class="pointer-events-none absolute top-0 left-0"
	{width}
	{height}
	fill="none"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	{#each edges as edge (`${edge.from}-${edge.to}`)}
		<path
			class="motion-safe:transition-opacity"
			d={path(edge.points)}
			stroke={color(edge.line)}
			stroke-width="6"
			stroke-dasharray={edge.pending ? '1 11' : undefined}
			opacity={opacity(edge.line)}
		/>
	{/each}
	<g fill="var(--color-base-100)" stroke-width="3.5">
		{#each nodes as node (node.hash)}
			<g class="motion-safe:transition-opacity" opacity={opacity(node.line)}>
				{#if node.parents.length > 1}
					<rect
						x={x(node.lane) - 11}
						y={y(node.row) - 10}
						width="22"
						height="20"
						rx="10"
						stroke="var(--color-base-content)"
					/>
				{:else if node.hash === head}
					<circle cx={x(node.lane)} cy={y(node.row)} r="9" stroke="var(--color-base-content)" />
					<circle
						cx={x(node.lane)}
						cy={y(node.row)}
						r="3.5"
						fill={color(node.line)}
						stroke="none"
					/>
				{:else}
					<circle
						cx={x(node.lane)}
						cy={y(node.row)}
						r="6.5"
						stroke={color(node.line)}
						stroke-dasharray={node.pending ? '3 3' : undefined}
					/>
				{/if}
			</g>
		{/each}
	</g>
</svg>
