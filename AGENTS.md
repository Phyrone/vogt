# AGENTS.md

Guidance for AI agents working in this repository, whichever agent is running. `CLAUDE.md` and `GEMINI.md` are symlinks to this file, so this is the one place to edit.

## Project status

VOGT (Version & Operations GUI Tool) is a desktop app: a SvelteKit frontend in `src/` inside a Tauri 2 shell in `src-tauri/`.

The app shell exists: routes, a title bar with workspace and repository switchers, and a registry of workspaces and repositories kept in SQLite. **History is real, the working tree is not:** [src-tauri/src/history.rs](src-tauri/src/history.rs) reads commits, branches, tags and stashes with `git2`, assigns every commit to a line and lays the graph out in Rust ([src-tauri/src/layout.rs](src-tauri/src/layout.rs)). Every registered repository is indexed in the background into its own SQLite cache file ([src-tauri/src/cache.rs](src-tauri/src/cache.rs), [src-tauri/src/index.rs](src-tauri/src/index.rs)); a `notify` watcher on its git directory keeps it current and the frontend hears `repo-changed`. The history reads the rows it shows straight from that cache file; [src/lib/repo/history.ts](src/lib/repo/history.ts) turns the summary and those rows into the `Repository` the interface draws. Working changes are not read yet, and the commit composer, pull, push and the commit actions do nothing to a real repository. Outside the Tauri shell every repository shows `src/lib/repo/mock.ts`.

## Architecture

- **The frontend is a pure SPA.** SSR and prerendering are off and `adapter-static` builds to `build/`, which Tauri bundles. There is no Node server at runtime, so server-only SvelteKit files (`+page.server.ts`, `+layout.server.ts`, `+server.ts`, form actions) never run.
- **Everything that needs the system lives in Rust** as a Tauri command in `src-tauri/src/`, called from the frontend with `invoke` from `@tauri-apps/api/core`. The databases are the exception; see below.
- **Two databases, two owners.** The app's own SQLite file holds the registry: [src/lib/db/schema.ts](src/lib/db/schema.ts), `db` from [src/lib/db/index.ts](src/lib/db/index.ts), opened by `src/hooks.client.ts`, written by the frontend through Drizzle and `tauri-plugin-sql`. After a change run `pnpm db:generate --name <what>`; the `sql_migrations!` macro in `crates/vogt-macros` compiles every file in `drizzle/` into the app and the Rust side applies the new ones on the next start. Never edit a migration that has shipped.
- **Every repository has its own cache file** under the app cache directory (`workspace/<workspace>/repo/<repo>.db`). Its schema is [src/lib/db/repo-schema.ts](src/lib/db/repo-schema.ts); drizzle-kit generates the migrations into `drizzle-repo/` (`pnpm db:repo:generate --name <what>`) and [src-tauri/src/cache.rs](src-tauri/src/cache.rs) applies them when a cache file is created. The Rust side owns every write; the frontend reads the laid-out rows from the same file through `tauri-plugin-sql` ([src/lib/db/repo.ts](src/lib/db/repo.ts)).
- **The Rust queries are sqlx macros**, checked against a database built from `drizzle-repo/`; `pnpm db:repo:prepare` rebuilds it and refreshes the committed `.sqlx` cache. A schema change that the Rust side misses therefore fails to compile, and `.sqlx` must be regenerated whenever a query changes.
- **Types that cross into the webview are generated from the Rust structs** with ts-rs into [src/lib/bindings/](src/lib/bindings/); `cargo test` refreshes them. Never edit them by hand, and add `#[derive(TS)] #[ts(export)]` to a new struct that `invoke` or an event carries.
- **`notify` keeps the caches current.** [src-tauri/src/index.rs](src-tauri/src/index.rs) watches each registered repository's refs and HEAD, walks what is new into its cache in batches, lays the graph out again when the reachable commits changed, and emits `repo-changed` with its progress. The walk is resumable through the cache's `pending` table. `sync_repositories` tells the Rust side which repositories are registered; the root layout calls it after loading them.
- A new command works only once it is listed in `tauri::generate_handler![...]` in [src-tauri/src/lib.rs](src-tauri/src/lib.rs).
- A plugin or core API works only once its permission is granted in [src-tauri/capabilities/default.json](src-tauri/capabilities/default.json); without it the call is rejected at runtime, not at build time.
- Frontend code is Svelte 5 with runes and TypeScript in strict mode.
- **Routes carry ids, never paths or names:** `/workspaces/[workspace]` lists a workspace's repositories and `/workspaces/[workspace]/repos/[repo]/<view>` is one view of a repository (`history` so far). The ids are the primary keys of the `workspace` and `repo` tables. The one exception is `?worktree=<path>` on a repository route, which picks one of the worktrees git reports for it; without it the registered directory is shown.
- **Layouts load, pages render:** the root `+layout.ts` loads the workspaces, each `[param]` layout resolves its id from the parent data or fails with a 404, and the repository layout creates the `RepoSession` its pages share. Build links with `resolve` from `$app/paths`.
- **The frontend reaches the registry only through [src/lib/registry/api.ts](src/lib/registry/api.ts)**, which answers from the database in the Tauri shell and from `mock.ts` outside it, so every route also renders in a plain browser and in the end-to-end tests. After a change, `invalidate(REGISTRY)`.
- **Every page draws its own title bar** — `Toolbar`, or the bare `TitleBar` — because the window has no native one.

## Commands

`pnpm dev` and `pnpm build` drive the **whole desktop app** through Tauri, including the Rust build. The frontend alone is `pnpm vite:dev` and `pnpm vite:build`.

Verify a change with all four checks:

```bash
pnpm check                      # svelte-check, frontend types
pnpm lint                       # prettier --check and eslint; fix formatting with pnpm format
pnpm test                       # unit tests with coverage, Rust tests, then end-to-end tests
cargo check --workspace         # Rust; run cargo from the repository root
```

- The dev server is pinned to port 1420 and fails instead of picking another one. A leftover `pnpm dev` is the usual cause.
- Prettier and ESLint skip `src-tauri/` and the agent tooling directories; Rust is formatted with `cargo fmt`. `src/lib/bindings/`, `drizzle/` and `drizzle-repo/` are generated and skipped too.
- Schema changes: `pnpm db:generate --name <what>` for the registry, `pnpm db:repo:generate --name <what>` for the repository caches.
- After changing one of the sqlx queries, run `pnpm db:repo:prepare` (it needs `cargo sqlx` installed) and commit the refreshed `.sqlx/`; builds otherwise compile against the committed query cache. `cargo test` refreshes the ts-rs bindings in `src/lib/bindings/`.
- `pnpm test:unit` is Vitest in watch mode and always collects coverage; add `--run` for a single pass. `*.svelte.spec.ts` files run in headless Chromium, every other `*.spec.ts` in node.
- `pnpm test:e2e` is Playwright over `*.e2e.ts` files; it builds and serves the frontend itself, without the Tauri shell, so `invoke` calls have no backend there.
- `pnpm test:rust` runs the Rust tests with `cargo nextest`, not `cargo test`; CI runs them on Linux, Windows and macOS.
- `pnpm test:unit:ui` and `pnpm test:e2e:ui` open the interactive runners.
- With `CI` set, both runners also write JUnit files to `test-results/` and GitHub annotations; coverage lands in `coverage/`.

## Conventions

- **Version** is declared three times — `package.json`, `src-tauri/tauri.conf.json` and `[workspace.package]` in the root `Cargo.toml`. Change all three together.
- **Cargo workspace:** the root `Cargo.toml` owns the shared package fields, every dependency version with its features, and the build profiles. Crates take them with `<key>.workspace = true`; add a new dependency to `[workspace.dependencies]` first.
- **Commits:** every commit follows [docs/commit-conventions.md](docs/commit-conventions.md) — a gitmoji + Conventional Commits header and the why in the body. The `committing` skill must be used before running `git commit`.
- **Skills** live in `.agents/skills/`; the per-agent directories (`.claude/skills/`, `.hermes/skills/`) hold symlinks into it. The `skills` CLI (`pnpm skills`) manages the ones in `skills-lock.json`; `committing` is local to this repository and edited by hand.

## Agent skills

### Issue tracker

Issues live in GitHub Issues on `Phyrone/vogt` (the `origin` remote), driven with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, unrenamed. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `GLOSSARY.md` and `docs/adr/`. See `docs/agents/domain.md`.

## Working with Svelte

You are able to use the Svelte MCP server, where you have access to comprehensive Svelte 5 and SvelteKit documentation. Here's how to use the available tools effectively:

## Available Svelte MCP Tools:

### 1. list-sections

Use this FIRST to discover all available documentation sections. Returns a structured list with titles, use_cases, and paths.
When asked about Svelte or SvelteKit topics, ALWAYS use this tool at the start of the chat to find relevant sections.

### 2. get-documentation

Retrieves full documentation content for specific sections. Accepts single or multiple sections.
After calling the list-sections tool, you MUST analyze the returned documentation sections (especially the use_cases field) and then use the get-documentation tool to fetch ALL documentation sections that are relevant for the user's task.

### 3. svelte-autofixer

Analyzes Svelte code and returns issues and suggestions.
You MUST use this tool whenever writing Svelte code before sending it to the user. Keep calling it until no issues or suggestions are returned.

### 4. playground-link

Generates a Svelte Playground link with the provided code.
After completing the code, ask the user if they want a playground link. Only call this tool after user confirmation and NEVER if code was written to files in their project.
