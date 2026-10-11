import type { Repository } from './types';

/** Synthetic repository for building the interface before any Tauri command exists. */
export const repository: Repository = {
	name: 'vogt',
	user: 'Phyrone',
	trunk: 'main',
	headLine: 'graph',
	head: 'a41f9c2',
	lines: {
		main: { id: 'main', name: 'main', badge: 'M', color: 'primary', remote: true },
		graph: {
			id: 'graph',
			name: 'feat/commit-graph',
			badge: 'G',
			color: 'secondary',
			remote: false
		},
		composer: {
			id: 'composer',
			name: 'feat/gitmoji-composer',
			badge: 'C',
			color: 'accent',
			remote: true
		},
		rebase: { id: 'rebase', name: 'fix/rebase-abort', badge: 'R', color: 'warning', remote: true },
		open: {
			id: 'open',
			name: 'feat/repo-open',
			badge: 'O',
			color: 'info',
			remote: false,
			merged: true
		}
	},
	commits: [
		{
			hash: 'a41f9c2',
			subject: '✨ feat(graph): draw merge interchanges',
			body: 'Merges were drawn as plain nodes, which hid which line was merged into which.',
			author: 'Phyrone',
			when: '12 min',
			line: 'graph',
			parents: ['7d03b1e'],
			pushed: false,
			files: [
				{ path: 'src/lib/graph/Interchange.svelte', added: 84, removed: 0 },
				{ path: 'src/lib/graph/layout.ts', added: 31, removed: 12 },
				{ path: 'src/lib/graph/Graph.svelte', added: 4, removed: 2 },
				{ path: 'src/lib/graph/layout.spec.ts', added: 2, removed: 0 }
			]
		},
		{
			hash: '7d03b1e',
			subject: '♻️ refactor(graph): lay out lanes in rust',
			author: 'Phyrone',
			when: '1 h',
			line: 'graph',
			parents: ['c2e7d55'],
			pushed: false,
			files: [
				{ path: 'src-tauri/src/graph.rs', added: 142, removed: 0 },
				{ path: 'src/lib/graph/layout.ts', added: 9, removed: 97 }
			]
		},
		{
			hash: 'e5c8a70',
			subject: '✨ feat(composer): suggest scopes from history',
			author: 'Mira',
			when: '3 h',
			line: 'composer',
			parents: ['5f1a0b9'],
			pushed: true,
			files: [{ path: 'src/lib/composer/scopes.ts', added: 58, removed: 3 }]
		},
		{
			hash: '19b6f4d',
			subject: '🐛 fix(rebase): restore HEAD after abort',
			author: 'Jonas',
			when: '5 h',
			line: 'rebase',
			parents: ['d07e1fa'],
			pushed: true,
			files: [{ path: 'src-tauri/src/rebase.rs', added: 17, removed: 5 }]
		},
		{
			hash: 'c2e7d55',
			subject: '🔀 merge: feat/repo-open into main',
			author: 'Phyrone',
			when: 'yesterday',
			line: 'main',
			parents: ['3a9d7c6', '8b44e21'],
			refs: [{ kind: 'remote', label: 'origin/main' }],
			pushed: true,
			files: []
		},
		{
			hash: '8b44e21',
			subject: '✨ feat(repo): open a repository from disk',
			author: 'Mira',
			when: 'yesterday',
			line: 'open',
			parents: ['3a9d7c6'],
			pushed: true,
			files: [
				{ path: 'src-tauri/src/repo.rs', added: 96, removed: 0 },
				{ path: 'src/routes/+page.svelte', added: 22, removed: 140 }
			]
		},
		{
			hash: '3a9d7c6',
			subject: '✅ test(graph): cover octopus merges',
			author: 'Jonas',
			when: '2 d',
			line: 'main',
			parents: ['5f1a0b9'],
			pushed: true,
			files: [{ path: 'src/lib/graph/layout.spec.ts', added: 41, removed: 0 }]
		},
		{
			hash: '5f1a0b9',
			subject: '✨ feat(composer): gitmoji picker',
			author: 'Mira',
			when: '2 d',
			line: 'main',
			parents: ['d07e1fa'],
			pushed: true,
			files: [{ path: 'src/lib/composer/Gitmoji.svelte', added: 73, removed: 0 }]
		},
		{
			hash: 'd07e1fa',
			subject: '✨ feat(graph): walk commits in rust',
			author: 'Phyrone',
			when: '3 d',
			line: 'main',
			parents: ['fde9c95'],
			pushed: true,
			files: [{ path: 'src-tauri/src/graph.rs', added: 64, removed: 0 }]
		},
		{
			hash: 'fde9c95',
			subject: '🔧 chore(agents): add the daisyui skill',
			author: 'Phyrone',
			when: '4 d',
			line: 'main',
			parents: ['1f9de47'],
			pushed: true,
			files: [{ path: 'skills-lock.json', added: 6, removed: 0 }]
		},
		{
			hash: '1f9de47',
			subject: '➕ build(ui): add tailwind css and daisyui',
			author: 'Phyrone',
			when: '4 d',
			line: 'main',
			parents: ['c4ecbb7'],
			pushed: true,
			files: [
				{ path: 'package.json', added: 4, removed: 0 },
				{ path: 'src/app.css', added: 3, removed: 0 }
			]
		},
		{
			hash: 'c4ecbb7',
			subject: '✨ feat(assets): add the app icon',
			author: 'Phyrone',
			when: '5 d',
			line: 'main',
			parents: ['aeb1e9d'],
			refs: [{ kind: 'tag', label: 'v0.0.1' }],
			pushed: true,
			files: [{ path: 'icon.svg', added: 12, removed: 0 }]
		},
		{
			hash: 'aeb1e9d',
			subject: 'Add GNU GPL v3 license',
			author: 'Phyrone',
			when: '6 d',
			line: 'main',
			parents: ['d613a3b'],
			pushed: true,
			files: [{ path: 'LICENSE', added: 674, removed: 0 }]
		},
		{
			hash: 'd613a3b',
			subject: '♻️ refactor(build): make the repository root a cargo workspace',
			author: 'Phyrone',
			when: '6 d',
			line: 'main',
			parents: [],
			pushed: true,
			files: [
				{ path: 'Cargo.toml', added: 28, removed: 0 },
				{ path: 'src-tauri/Cargo.toml', added: 9, removed: 21 }
			]
		}
	],
	changes: [
		{ path: 'src/lib/graph/Interchange.svelte', added: 84, removed: 0, staged: true },
		{ path: 'src/lib/graph/layout.ts', added: 31, removed: 12, staged: true },
		{ path: 'src-tauri/src/graph.rs', added: 6, removed: 2, staged: false }
	],
	stashes: ['WIP on feat/commit-graph', 'WIP on main'],
	tags: ['v0.0.1']
};
