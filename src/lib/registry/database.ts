import { and, eq, sql } from 'drizzle-orm';
import { invoke } from '@tauri-apps/api/core';
import { closeRepoDatabase } from '#lib/db/repo.ts';
import { db } from '#lib/db/index.ts';
import { repo, workspace } from '#lib/db/schema.ts';
import type { Repo, Workspace } from './types.ts';

/** The registry kept in the app's database. */
export const database = {
	async listWorkspaces(): Promise<Workspace[]> {
		let workspaces = await db
			.select()
			.from(workspace)
			.orderBy(sql`rowid`);
		// there is always one to show
		if (!workspaces.length) {
			workspaces = await db.insert(workspace).values({ name: 'Default' }).returning();
		}
		const repos = await db
			.select()
			.from(repo)
			.orderBy(sql`rowid`);
		return workspaces.map((w) => ({ ...w, repos: repos.filter((r) => r.workspaceId === w.id) }));
	},

	async createWorkspace(name: string): Promise<Workspace> {
		if (!name.trim()) throw 'A workspace needs a name';
		const [created] = await db.insert(workspace).values({ name: name.trim() }).returning();
		return { ...created!, repos: [] };
	},

	/** Adds the directory at `path` to a workspace; a path already in it keeps its id. */
	async addRepo(workspaceId: string, path: string): Promise<Repo> {
		const [existing] = await db
			.select()
			.from(repo)
			.where(and(eq(repo.workspaceId, workspaceId), eq(repo.path, path)));
		if (existing) return existing;
		const name = path.split(/[\\/]/).findLast(Boolean) ?? path;
		const [added] = await db.insert(repo).values({ workspaceId, name, path }).returning();
		return added!;
	},

	/** Takes a repository out of a workspace and drops the cache built for it. */
	async removeRepo(workspaceId: string, id: string): Promise<void> {
		await db.delete(repo).where(and(eq(repo.workspaceId, workspaceId), eq(repo.id, id)));
		const cachePath = await invoke<string>('forget_repo', { workspace: workspaceId, repo: id });
		await closeRepoDatabase(cachePath);
	},

	/** Tells the backend which repositories to watch and keep in sync. */
	async syncRepositories(workspaces: Workspace[]): Promise<void> {
		await invoke('sync_repositories', {
			list: workspaces.flatMap((workspace) =>
				workspace.repos.map((repo) => ({
					id: repo.id,
					workspace: workspace.id,
					path: repo.path
				}))
			)
		});
	}
};
