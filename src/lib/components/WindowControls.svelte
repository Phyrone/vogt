<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';

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
		aria-label="Minimize"
		title="Minimize"
		onclick={() => appWindow.minimize()}
	>
		<svg class="size-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
			<path d="M3.5 8h9" stroke-linecap="round" />
		</svg>
	</button>
	<button
		type="button"
		class="btn btn-square btn-ghost btn-sm"
		aria-label={maximized ? 'Restore' : 'Maximize'}
		title={maximized ? 'Restore' : 'Maximize'}
		onclick={() => appWindow.toggleMaximize()}
	>
		<svg
			class="size-4"
			viewBox="0 0 16 16"
			fill="none"
			stroke="currentColor"
			stroke-width="1.5"
			stroke-linejoin="round"
		>
			{#if maximized}
				<path d="M3.5 5.5h7v7h-7zM5.5 5.5v-2h7v7h-2" />
			{:else}
				<path d="M3.5 3.5h9v9h-9z" />
			{/if}
		</svg>
	</button>
	<button
		type="button"
		class="btn btn-square btn-ghost btn-sm hover:border-error hover:bg-error hover:text-error-content"
		aria-label="Close"
		title="Close"
		onclick={() => appWindow.close()}
	>
		<svg class="size-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
			<path d="m4 4 8 8M12 4l-8 8" stroke-linecap="round" />
		</svg>
	</button>
</div>
