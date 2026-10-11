import { sqliteTable, text, unique } from 'drizzle-orm/sqlite-core';

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
