import type { Repo, Workspace } from './types.ts';

// Stands in for the Rust registry when the page runs outside the Tauri shell.
const workspaces: Workspace[] = [
	{
		id: 'ws-personal',
		name: 'Personal',
		repos: [
			{ id: 'repo-vogt', name: 'vogt', path: '~/code/vogt' },
			{ id: 'repo-dotfiles', name: 'dotfiles', path: '~/code/dotfiles' },
			{ id: 'repo-notes', name: 'notes', path: '~/documents/notes' }
		]
	},
	{
		id: 'ws-work',
		name: 'Work',
		repos: [
			{ id: 'repo-api', name: 'api', path: '~/work/api' },
			{ id: 'repo-web', name: 'web', path: '~/work/web' }
		]
	},
	{ id: 'ws-empty', name: 'Experiments', repos: [] }
];
let created = 0;

function find(id: string): Workspace {
	const workspace = workspaces.find((w) => w.id === id);
	if (!workspace) throw 'This workspace no longer exists';
	return workspace;
}

export const mock = {
	listWorkspaces: (): Workspace[] => structuredClone(workspaces),
	createWorkspace(name: string): Workspace {
		if (!name.trim()) throw 'A workspace needs a name';
		const workspace = { id: `ws-${++created}`, name: name.trim(), repos: [] };
		workspaces.push(workspace);
		return structuredClone(workspace);
	},
	addRepo(workspace: string, path: string): Repo {
		const repo = { id: `repo-${++created}`, name: path.split('/').at(-1) ?? path, path };
		find(workspace).repos.push(repo);
		return { ...repo };
	},
	removeRepo(workspace: string, repo: string): void {
		const target = find(workspace);
		target.repos = target.repos.filter((r) => r.id !== repo);
	},
	syncRepositories(): void {},
	pickDirectory: (): string => `~/code/project-${created + 1}`
};
