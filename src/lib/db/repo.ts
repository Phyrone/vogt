import Database from '@tauri-apps/plugin-sql';
import { and, eq, gte, lt } from 'drizzle-orm';
import { drizzle } from 'drizzle-orm/sqlite-proxy';
import type { SqliteRemoteDatabase } from 'drizzle-orm/sqlite-proxy';
import type { ViewInfo } from '#lib/bindings/ViewInfo.ts';
import { proxy } from './index.ts';
import * as schema from './repo-schema.ts';

/** A repository's own cache database, opened for reading. */
export interface RepoDatabase {
	/** The plugin's connection, for closing it when the repository is removed. */
	sqlite: Database;
	db: SqliteRemoteDatabase<typeof schema>;
}

/** One commit of a view, as the tables hold it. */
export interface RepoRow {
	row: number;
	lane: number;
	hash: string;
	subject: string;
	author: string;
	/** Seconds since the Unix epoch. */
	time: number;
	body: string | null;
	/** JSON array of parent hashes. */
	parents: string;
	line: string;
	/** JSON array of `{ kind, label }`. */
	refs: string;
	pushed: number;
}

/** A stretch of a line between two rows, as the tables hold it. */
export interface RepoEdge {
	from: number;
	to: number;
	line: string;
	/** JSON array of `[lane, row]` corner points. */
	points: string;
}

const open = new Map<string, Promise<RepoDatabase>>();

/** Opens the cache file at `path` (once per repository id) for the frontend to query. */
export function loadRepoDatabase(id: string, path: string): Promise<RepoDatabase> {
	let pending = open.get(id);
	if (!pending) {
		pending = (async () => {
			const sqlite = await Database.load(`sqlite:${path}`);
			return { sqlite, db: drizzle(proxy(sqlite), { schema }) };
		})();
		open.set(id, pending);
		// a failure is often passing (a locked file, a virus scanner), so the next call tries again
		const loading = pending;
		loading.catch(() => {
			if (open.get(id) === loading) open.delete(id);
		});
	}
	return pending;
}

/** Closes the cache of a repository that was removed. */
export async function closeRepoDatabase(id: string): Promise<void> {
	const pending = open.get(id);
	open.delete(id);
	if (pending) {
		await pending.then((repo) => repo.sqlite.close()).catch(() => {});
	}
}

/** The rows `from..to` of a view and the edges that cross them. */
export async function window(
	repo: RepoDatabase,
	view: ViewInfo,
	from: number,
	to: number
): Promise<{ rows: RepoRow[]; edges: RepoEdge[] }> {
	const rows = await repo.db
		.select({
			row: schema.node.row,
			lane: schema.node.lane,
			hash: schema.node.hash,
			subject: schema.commit.subject,
			author: schema.commit.author,
			time: schema.commit.time,
			body: schema.commit.body,
			parents: schema.commit.parents,
			line: schema.node.line,
			refs: schema.node.refs,
			pushed: schema.node.pushed
		})
		.from(schema.node)
		.innerJoin(schema.commit, eq(schema.node.hash, schema.commit.hash))
		.where(
			and(eq(schema.node.viewId, view.id), gte(schema.node.row, from), lt(schema.node.row, to))
		)
		.orderBy(schema.node.row);
	const edges = await repo.db
		.select({
			from: schema.edge.from,
			to: schema.edge.to,
			line: schema.edge.line,
			points: schema.edge.points
		})
		.from(schema.edge)
		.where(
			and(eq(schema.edge.viewId, view.id), lt(schema.edge.from, to), gte(schema.edge.to, from))
		);
	return { rows, edges };
}

/** The row the commit `hash` sits in, of the unmasked view. */
export async function locate(repo: RepoDatabase, hash: string): Promise<number | null> {
	const [found] = await repo.db
		.select({ row: schema.node.row })
		.from(schema.node)
		.where(and(eq(schema.node.viewId, 'all'), eq(schema.node.hash, hash)))
		.limit(1);
	return found?.row ?? null;
}
