import { resolve } from '$app/paths';

/** The app's settings. */
export function appSettingsPath() {
	return resolve('/settings');
}

/** One repository's settings; `worktree` keeps the route on the worktree in view. */
export function repoSettingsPath(workspace: string, repo: string, worktree?: string | null) {
	return worktree
		? resolve(
				`/workspaces/[workspace]/repos/[repo]/settings?worktree=${encodeURIComponent(worktree)}`,
				{ workspace, repo }
			)
		: resolve('/workspaces/[workspace]/repos/[repo]/settings', { workspace, repo });
}
