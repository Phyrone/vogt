import { listWorkspaces, REGISTRY, syncRepositories } from '#lib/registry/api.ts';
import type { LayoutLoad } from './$types';

// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;
export const prerender = false;

export const load: LayoutLoad = async ({ depends }) => {
	depends(REGISTRY);
	const workspaces = await listWorkspaces();
	// every registered repository is watched and indexed while the app runs
	await syncRepositories(workspaces);
	return { workspaces };
};
