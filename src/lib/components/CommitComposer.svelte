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

	let type = $state('feat');
	let scope = $state('');
	let subject = $state('');
	let body = $state('');

	const staged = $derived(changes.filter((c) => c.staged).length);
	const header = $derived(
		`${types.find((t) => t.name === type)?.emoji} ${type}${scope && `(${scope})`}: ${subject.trim()}`
	);
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

	<fieldset class="mt-auto">
		<legend class="mb-1.5 caps text-base-content/70">Type</legend>
		<div class="flex flex-wrap gap-1.5">
			{#each types as t (t.name)}
				<input
					type="radio"
					class="btn btn-sm checked:[--btn-color:var(--color-neutral)] checked:[--btn-fg:var(--color-neutral-content)]"
					name="type"
					value={t.name}
					aria-label="{t.emoji} {t.name}"
					bind:group={type}
				/>
			{/each}
		</div>
	</fieldset>
	<fieldset class="mt-2">
		<legend class="mb-1.5 caps text-base-content/70">Scope</legend>
		<div class="flex flex-wrap gap-1.5">
			{#each ['', ...scopes] as s (s)}
				<input
					type="radio"
					class="btn btn-sm checked:[--btn-color:var(--color-neutral)] checked:[--btn-fg:var(--color-neutral-content)]"
					name="scope"
					value={s}
					aria-label={s || 'none'}
					bind:group={scope}
				/>
			{/each}
		</div>
	</fieldset>
	<label class="mt-2 caps text-base-content/70" for="subject">Subject</label>
	<label class="input w-full">
		<input id="subject" type="text" placeholder="what this commit does" bind:value={subject} />
		<span
			class={[
				'font-mono text-xs',
				subject.length > SUBJECT_LIMIT ? 'text-error' : 'text-base-content/70'
			]}>{subject.length} / {SUBJECT_LIMIT}</span
		>
	</label>
	<p class="truncate font-mono text-xs text-base-content/70" title={header}>{header}</p>
	<textarea
		class="textarea h-20 w-full resize-none"
		placeholder="Why this change was needed"
		aria-label="Description"
		bind:value={body}></textarea>
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
