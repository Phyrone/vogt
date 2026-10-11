<script lang="ts">
	import GitCommitHorizontal from '@lucide/svelte/icons/git-commit-horizontal';
	import { lineStyle, type FileChange, type Line } from '#lib/repo/types.ts';

	interface Props {
		changes: FileChange[];
		line: Line;
		/** Scopes already used in this repository, most recent first. */
		scopes: string[];
		oncommit: (subject: string, body: string) => void;
	}

	let { changes = $bindable(), line, scopes, oncommit }: Props = $props();

	const types = [
		{ emoji: '✨', name: 'feat' },
		{ emoji: '🐛', name: 'fix' },
		{ emoji: '♻️', name: 'refactor' },
		{ emoji: '✅', name: 'test' },
		{ emoji: '📝', name: 'docs' },
		{ emoji: '🔧', name: 'chore' }
	];
	const SUBJECT_LIMIT = 50;

	// how this repository writes commit headers; will come from the per-repository settings
	const style = 'gitmoji' as 'plain' | 'conventional' | 'gitmoji';
	let type = $state('feat');
	let scope = $state('');
	let subject = $state('');
	let body = $state('');

	const staged = $derived(changes.filter((c) => c.staged).length);
	const header = $derived.by(() => {
		if (style === 'plain') return subject.trim();
		const emoji = style === 'gitmoji' ? `${types.find((t) => t.name === type)?.emoji} ` : '';
		return `${emoji}${type}${scope.trim() && `(${scope.trim()})`}: ${subject.trim()}`;
	});
	const ready = $derived(staged > 0 && subject.trim().length > 0);

	function stageAll() {
		const all = staged < changes.length;
		for (const change of changes) change.staged = all;
	}

	function onsubmit(event: SubmitEvent) {
		event.preventDefault();
		if (!ready) return;
		oncommit(header, body.trim());
		subject = '';
		body = '';
	}
</script>

<form class="flex min-h-full flex-col gap-2 p-4" {onsubmit}>
	<div class="flex items-center justify-between">
		<h2 class="caps text-base-content/70">
			Changes · {changes.length}
			{changes.length === 1 ? 'file' : 'files'}
		</h2>
		<button type="button" class="btn btn-ghost btn-xs" onclick={stageAll}>
			{staged < changes.length ? 'Stage all' : 'Unstage all'}
		</button>
	</div>
	<ul class="mb-4">
		{#each changes as change (change.path)}
			<li>
				<label class="flex h-8 cursor-pointer items-center gap-2.5">
					<input type="checkbox" class="checkbox checkbox-sm" bind:checked={change.staged} />
					<span
						class={['min-w-0 flex-1 truncate', !change.staged && 'text-base-content/70']}
						title={change.path}>{change.path}</span
					>
					<span class="font-mono text-xs text-success">+{change.added}</span>
					<span class="font-mono text-xs text-error">−{change.removed}</span>
				</label>
			</li>
		{/each}
	</ul>
	<div
		class="mt-auto overflow-hidden rounded-field border border-base-content/20 bg-base-100 focus-within:border-base-content"
	>
		<div class="flex flex-wrap items-center gap-x-2 gap-y-1.5 px-3 pt-2.5">
			{#if style !== 'plain'}
				<span class="flex shrink-0 items-center font-mono text-xs">
					<select
						class="cursor-pointer rounded-selector bg-base-200 py-1 pr-1 pl-1.5 outline-offset-0"
						aria-label="Type"
						bind:value={type}
					>
						{#each types as t (t.name)}
							<option value={t.name}>{style === 'gitmoji' ? `${t.emoji} ` : ''}{t.name}</option>
						{/each}
					</select>
					<span class="pl-1 text-base-content/50" aria-hidden="true">(</span>
					<input
						type="text"
						class="bg-transparent text-center outline-none placeholder:text-base-content/50"
						list="commit-scopes"
						style:width="{Math.max(scope.length, 5) + 3}ch"
						placeholder="scope"
						aria-label="Scope"
						bind:value={scope}
					/>
					<span class="text-base-content/50" aria-hidden="true">):</span>
					<datalist id="commit-scopes">
						{#each scopes as s (s)}<option value={s}></option>{/each}
					</datalist>
				</span>
			{/if}
			<input
				type="text"
				class="min-w-32 flex-1 bg-transparent font-semibold outline-none"
				placeholder="Summary of the change"
				aria-label="Subject"
				bind:value={subject}
			/>
			<span
				class={[
					'shrink-0 font-mono text-xs',
					subject.length > SUBJECT_LIMIT ? 'text-error' : 'text-base-content/70'
				]}>{subject.length}</span
			>
		</div>
		<textarea
			class="block h-24 w-full resize-none bg-transparent px-3 py-2 outline-none"
			placeholder="Why this change was needed"
			aria-label="Description"
			bind:value={body}></textarea>
	</div>
	<button
		type="submit"
		class="btn border-0 bg-(--line) signage text-base font-semibold text-(--line-content)"
		style={lineStyle(line)}
		disabled={!ready}
	>
		<GitCommitHorizontal class="size-5" aria-hidden="true" />
		Commit {staged}
		{staged === 1 ? 'file' : 'files'} to {line.name}
	</button>
</form>
