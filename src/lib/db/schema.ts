import { blob, integer, primaryKey, sqliteTable, text, unique } from 'drizzle-orm/sqlite-core';

const id = () =>
	text('id')
		.primaryKey()
		.$defaultFn(() => crypto.randomUUID());

export const workspace = sqliteTable('workspace', {
	id: id(),
	name: text('name').notNull()
});

export const repo = sqliteTable(
	'repo',
	{
		id: id(),
		workspaceId: text('workspace_id')
			.notNull()
			.references(() => workspace.id, { onDelete: 'cascade' }),
		name: text('name').notNull(),
		path: text('path').notNull()
	},
	(table) => [unique().on(table.workspaceId, table.path)]
);

/**
 * What the graph needs of a commit, remembered so that a history does not have to be read out of
 * git again. Written and read by the Rust side only, see `src-tauri/src/cache.rs`.
 */
export const commitNode = sqliteTable(
	'commit_node',
	{
		repoId: text('repo_id')
			.notNull()
			.references(() => repo.id, { onDelete: 'cascade' }),
		/** The 20 bytes of the commit's id. */
		hash: blob('hash', { mode: 'buffer' }).notNull(),
		/** Commit time in seconds since the Unix epoch. */
		time: integer('time').notNull(),
		/** The ids of the parents, 20 bytes each, first parent first. */
		parents: blob('parents', { mode: 'buffer' }).notNull()
	},
	(table) => [primaryKey({ columns: [table.repoId, table.hash] })]
);
