<script lang="ts">
	import { lineStyle, type Line } from '#lib/repo/types.ts';

	interface Props {
		line: Line;
		size?: 'sm' | 'md';
	}

	let { line, size = 'md' }: Props = $props();
</script>

<span
	class={[
		'inline-grid shrink-0 place-items-center rounded-[5px] bg-(--line) signage leading-none text-(--line-content)',
		size === 'sm' ? 'size-5 text-xs' : 'size-[22px] text-sm'
	]}
	style={lineStyle(line)}
	aria-hidden="true">{line.badge}</span
>
