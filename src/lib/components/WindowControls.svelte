<script lang="ts">
	import Copy from '@lucide/svelte/icons/copy';
	import Minus from '@lucide/svelte/icons/minus';
	import Square from '@lucide/svelte/icons/square';
	import X from '@lucide/svelte/icons/x';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { m } from '#lib/paraglide/messages.js';

	interface Props {
		maximized: boolean;
	}

	let { maximized }: Props = $props();

	const appWindow = getCurrentWindow();
</script>

<div class="flex shrink-0 items-center gap-0.5">
	<button
		type="button"
		class="btn btn-square btn-ghost btn-sm"
		aria-label={m['common.minimize']()}
		title={m['common.minimize']()}
		onclick={() => appWindow.minimize()}
	>
		<Minus class="size-4" aria-hidden="true" />
	</button>
	<button
		type="button"
		class="btn btn-square btn-ghost btn-sm"
		aria-label={maximized ? m['common.restore']() : m['common.maximize']()}
		title={maximized ? m['common.restore']() : m['common.maximize']()}
		onclick={() => appWindow.toggleMaximize()}
	>
		{#if maximized}<Copy class="size-3.5 -scale-x-100" aria-hidden="true" />{:else}<Square
				class="size-3.5"
				aria-hidden="true"
			/>{/if}
	</button>
	<button
		type="button"
		class="btn btn-square btn-ghost btn-sm hover:border-error hover:bg-error hover:text-error-content"
		aria-label={m['common.close']()}
		title={m['common.close']()}
		onclick={() => appWindow.close()}
	>
		<X class="size-4" aria-hidden="true" />
	</button>
</div>
