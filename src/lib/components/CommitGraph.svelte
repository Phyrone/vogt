<script lang="ts">
	import { graphWidth, INSET, PITCH, sliceGraph, type GraphLayout } from '#lib/graph/layout.ts';
	import type { Line } from '#lib/repo/types.ts';

	interface Props {
		layout: GraphLayout;
		/** The first and last row to draw; the rest of a long history is left out. */
		rows: [number, number];
		lines: Record<string, Line>;
		head: string;
		/** When set, every other line is dimmed. */
		focusLine: string | null;
	}

	let { layout, rows, lines, head, focusLine }: Props = $props();

	const shown = $derived(sliceGraph(layout, rows[0], rows[1]));

	const x = (lane: number) => INSET + lane * PITCH;
	const y = (row: number) => PITCH / 2 + row * PITCH;
	const color = (line: string) => `var(--color-${lines[line].color})`;
	const path = (points: [number, number][]) =>
		points.map(([lane, row], i) => `${i ? 'L' : 'M'}${x(lane)} ${y(row)}`).join('');
	const opacity = (line: string) => (focusLine !== null && focusLine !== line ? 0.25 : 1);
</script>

<svg
	class="pointer-events-none absolute top-0 left-0"
	width={graphWidth(layout.lanes)}
	height={layout.nodes.length * PITCH}
	fill="none"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	{#each shown.edges as edge (edge.from + edge.to)}
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
		{#each shown.nodes as node (node.hash)}
			<g class="motion-safe:transition-opacity" opacity={opacity(node.line)}>
				{#if node.kind === 'interchange'}
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
						stroke-dasharray={node.kind === 'pending' ? '3 3' : undefined}
					/>
				{/if}
			</g>
		{/each}
	</g>
</svg>
