import { and, eq, sql } from 'drizzle-orm';
import { invoke } from '@tauri-apps/api/core';
import { closeRepoDatabase } from '#lib/db/repo.ts';
import { db } from '#lib/db/index.ts';
import { repo, workspace } from '#lib/db/schema.ts';
import type { Repo, Workspace } from './types.ts';

/** What was handed to the backend about the registered repositories, one call after the other. */
let synced: Promise<unknown> = Promise.resolve();

/** Runs `task` once every sync and removal handed to the backend before it is done. */
function afterSyncs<T>(task: () => Promise<T>): Promise<T> {
	const done = synced.then(task);
	synced = done.catch(() => {});
	return done;
}

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
		// the connection goes first, so a Windows delete is not blocked by it
		await closeRepoDatabase(id);
		// a sync on its way may still list the repository and would open its cache again; the row
		// goes in the same step, so no later sync reads it after the cache was forgotten
		await afterSyncs(async () => {
			await invoke<string>('forget_repo', { workspace: workspaceId, repo: id });
			await db.delete(repo).where(and(eq(repo.workspaceId, workspaceId), eq(repo.id, id)));
		});
	},

	/**
	 * Tells the backend which repositories to watch and keep in sync. Each sync waits for the
	 * syncs and removals before it and only then reads the registry, so the backend ends on the
	 * newest list.
	 */
	syncRepositories(): Promise<void> {
		return afterSyncs(async () => {
			const repos = await db
				.select()
				.from(repo)
				.orderBy(sql`rowid`);
			const list = repos.map((r) => ({ id: r.id, workspace: r.workspaceId, path: r.path }));
			await invoke<void>('sync_repositories', { list });
		});
	}
};
