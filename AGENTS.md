# AGENTS.md

Guidance for AI agents working in this repository, whichever agent is running. `CLAUDE.md` and `GEMINI.md` are symlinks to this file, so this is the one place to edit.

## Project status

VOGT (Version & Operations GUI Tool) is a desktop app: a SvelteKit frontend in `src/` inside a Tauri 2 shell in `src-tauri/`.

**Everything under `src/` and `src-tauri/src/` is still the create-tauri-app template** — the `greet` command, the demo page and its styles are placeholders. Replace them; they set no convention.

## Architecture

- **The frontend is a pure SPA.** SSR and prerendering are off and `adapter-static` builds to `build/`, which Tauri bundles. There is no Node server at runtime, so server-only SvelteKit files (`+page.server.ts`, `+layout.server.ts`, `+server.ts`, form actions) never run.
- **Everything that needs the system lives in Rust** as a Tauri command in `src-tauri/src/`, called from the frontend with `invoke` from `@tauri-apps/api/core`.
- A new command works only once it is listed in `tauri::generate_handler![...]` in [src-tauri/src/lib.rs](src-tauri/src/lib.rs).
- A plugin or core API works only once its permission is granted in [src-tauri/capabilities/default.json](src-tauri/capabilities/default.json); without it the call is rejected at runtime, not at build time.
- Frontend code is Svelte 5 with runes and TypeScript in strict mode.

## Commands

`pnpm dev` and `pnpm build` drive the **whole desktop app** through Tauri, including the Rust build. The frontend alone is `pnpm vite:dev` and `pnpm vite:build`.

Verify a change with both checks:

```bash
pnpm check                      # svelte-check, frontend types
cd src-tauri && cargo check     # Rust; there is no Cargo workspace at the root
```

- The dev server is pinned to port 1420 and fails instead of picking another one. A leftover `pnpm dev` is the usual cause.
- No test runner, linter or formatter is configured yet. Match the surrounding style, and ask before adding one.

## Conventions

- **Version** is declared three times — `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`. Change all three together.
- **Commits:** every commit follows [docs/commit-conventions.md](docs/commit-conventions.md) — a gitmoji + Conventional Commits header and the why in the body. The `committing` skill must be used before running `git commit`.
- **Skills** live in `.agents/skills/`; the per-agent directories (`.claude/skills/`, `.hermes/skills/`) hold symlinks into it. The `skills` CLI (`pnpm skills`) manages the ones in `skills-lock.json`; `committing` is local to this repository and edited by hand.

## Agent skills

### Issue tracker

Issues live in GitHub Issues on `Phyrone/vogt` (the `origin` remote), driven with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, unrenamed. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `GLOSSARY.md` and `docs/adr/`. See `docs/agents/domain.md`.
