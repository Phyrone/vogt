import { sql } from 'drizzle-orm';
import { index, integer, primaryKey, sqliteTable, text } from 'drizzle-orm/sqlite-core';

/**
 * The tables of a repository's own cache database, one file per registered repository under the
 * app cache directory. The Rust side (`src-tauri/src/cache.rs`) creates and writes them; the
 * frontend only reads the rows the history shows.
 */

/** What git knows about a commit; read once and never changed. */
export const commit = sqliteTable(
	'commits',
	{
		hash: text('hash').primaryKey(),
		/** Commit time in seconds since the Unix epoch. */
		time: integer('time').notNull(),
		/** JSON array of parent hashes, first parent first. */
		parents: text('parents').notNull(),
		subject: text('subject').notNull(),
		author: text('author').notNull(),
		body: text('body'),
		/** The commit's parents have been read; the walk is done with it. */
		expanded: integer('expanded').notNull().default(0)
	},
	// the graph is read newest first, a page at a time
	(table) => [index('commit_time_hash').on(sql`${table.time} DESC`, table.hash)]
);

/** Where an interrupted walk resumes: commits whose parents are not read yet. */
export const pending = sqliteTable('pending', {
	hash: text('hash').primaryKey(),
	time: integer('time').notNull()
});

/** Progress and the current line list, as key/value pairs. */
export const meta = sqliteTable('meta', {
	key: text('key').primaryKey(),
	value: text('value').notNull()
});

/** One laid-out view: the unmasked history, or the commits of picked lines. */
export const view = sqliteTable('views', {
	/** `all`, or `mask:<the picked line ids as JSON>`. */
	id: text('id').primaryKey(),
	/** JSON array of line ids; empty means every line. */
	onlyLines: text('only_lines').notNull(),
	/** How many rows the view has. */
	total: integer('total').notNull(),
	/** How many lanes the whole view has. */
	lanes: integer('lanes').notNull(),
	/** The layout revision it was built from. */
	version: integer('version').notNull()
});

/** One row of a view: where a commit sits in the list and in the graph. */
export const node = sqliteTable(
	'node',
	{
		viewId: text('view_id').notNull(),
		row: integer('row').notNull(),
		hash: text('hash').notNull(),
		lane: integer('lane').notNull(),
		line: text('line').notNull(),
		pushed: integer('pushed').notNull(),
		/** JSON array of `{ kind, label }`. */
		refs: text('refs').notNull()
	},
	(table) => [
		primaryKey({ columns: [table.viewId, table.row] }),
		// the history locates a commit by hash across a whole view
		index('node_hash').on(table.viewId, table.hash),
		// `ahead` and the masked view filter a whole view by line
		index('node_line').on(table.viewId, table.line)
	]
);

/** A stretch of a line between two rows of a view. */
export const edge = sqliteTable(
	'edge',
	{
		viewId: text('view_id').notNull(),
		from: integer('from').notNull(),
		to: integer('to').notNull(),
		line: text('line').notNull(),
		/** JSON array of `[lane, row]` corner points. */
		points: text('points').notNull()
	},
	(table) => [primaryKey({ columns: [table.viewId, table.from, table.to] })]
);
