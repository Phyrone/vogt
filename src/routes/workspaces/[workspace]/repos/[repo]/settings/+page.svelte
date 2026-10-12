<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import { resolve } from '$app/paths';
	import LineBadge from '#lib/components/LineBadge.svelte';
	import { m } from '#lib/paraglide/messages.js';

	let { data } = $props();

	const session = $derived(data.session);
	const repo = $derived(data.repo);
	const worktree = $derived(data.worktree);
	const line = $derived(session.lines[session.headLine]);
	const historyHref = $derived(
		worktree
			? resolve(
					`/workspaces/[workspace]/repos/[repo]/history?worktree=${encodeURIComponent(worktree.path)}`,
					{ workspace: data.workspace.id, repo: repo.id }
				)
			: resolve('/workspaces/[workspace]/repos/[repo]/history', {
					workspace: data.workspace.id,
					repo: repo.id
				})
	);
</script>

<main class="min-h-0 flex-1 overflow-y-auto">
	<div class="mx-auto flex w-full max-w-2xl flex-col gap-8 px-6 py-10">
		<header class="flex flex-col gap-4">
			<a class="btn -ml-2 self-start btn-ghost btn-sm" href={historyHref}>
				<ArrowLeft class="size-4" aria-hidden="true" />{m['history.title']()}
			</a>
			<div>
				<h1 class="signage text-3xl">{m['common.repository_settings']()}</h1>
				<p class="text-base-content/70">{m['repository.how_it_works']()}</p>
			</div>
		</header>

		<section class="flex flex-col gap-3" aria-labelledby="repository">
			<h2 id="repository" class="caps text-base-content/60">{m['repository.section']()}</h2>
			<dl class="rounded-box border border-base-300">
				<div class="flex items-baseline gap-4 border-b border-base-300 px-4 py-3 last:border-b-0">
					<dt class="w-24 shrink-0 text-base-content/70">{m['repository.name']()}</dt>
					<dd class="min-w-0 truncate signage text-base">{repo.name}</dd>
				</div>
				<div class="flex items-baseline gap-4 border-b border-base-300 px-4 py-3 last:border-b-0">
					<dt class="w-24 shrink-0 text-base-content/70">{m['repository.folder']()}</dt>
					<dd class="min-w-0 truncate font-mono text-xs" title={repo.path}>{repo.path}</dd>
				</div>
				{#if worktree && worktree.path !== repo.path}
					<div class="flex items-baseline gap-4 border-b border-base-300 px-4 py-3 last:border-b-0">
						<dt class="w-24 shrink-0 text-base-content/70">{m['repository.worktree']()}</dt>
						<dd class="min-w-0 truncate font-mono text-xs" title={worktree.path}>
							{worktree.path}
						</dd>
					</div>
				{/if}
				{#if line}
					<div class="flex items-center gap-4 px-4 py-3">
						<dt class="w-24 shrink-0 text-base-content/70">{m['repository.checked_out']()}</dt>
						<dd class="flex min-w-0 items-center gap-2">
							<LineBadge {line} size="sm" />
							<span class="min-w-0 truncate signage text-base">{line.name}</span>
						</dd>
					</div>
				{/if}
			</dl>
		</section>

		<section class="flex flex-col gap-3" aria-labelledby="templates">
			<h2 id="templates" class="caps text-base-content/60">{m['repository.templates']()}</h2>
			<div
				class="rounded-box border border-dashed border-base-300 px-6 py-10 text-center text-balance"
			>
				<p class="signage text-base">{m['repository.nothing_yet']()}</p>
				<p class="mx-auto mt-1 max-w-prose text-base-content/70">
					{m['repository.templates_description']()}
				</p>
			</div>
		</section>
	</div>
</main>
