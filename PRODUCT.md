# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

VOGT is a desktop app for macOS, Windows and Linux: one shared interface rendered in a Tauri webview, not a per-OS native design language. Mobile is a possibility for later, not a current target; the `ios` and `android` scripts in `package.json` are template leftovers.

## Users

Developers working with git, across the whole range of fluency:

- **Beginners and people who avoid the git CLI**, who need the interface to be intuitive without first learning git's vocabulary.
- **Developers fluent in the git CLI**, who want speed: keyboard shortcuts and a command box for reaching any action without the mouse.

Both groups use the same app. Neither is a secondary audience.

## Product Purpose

VOGT (Version & Operations GUI Tool) is an open-source desktop client for git, inspired by GitKraken and intended as an open-source alternative to it.

It exists to make working with git and commits less draining. That covers the version-control work itself and the routine work that surrounds it in a repository.

"Operations" in the name means, as confirmed so far:

- **Commit authoring aids** — UI assistance for Conventional Commits and gitmoji.
- **Repo knowledge tooling** — working with ADRs and similar documents that live in the repository (later).
- **Forge integration** — pull requests, issues, reviews and CI status.

The first milestone is the point where VOGT is usable for daily work: open a repository, browse the commit graph and diffs, stage, commit with authoring aids, branch, merge, push and pull, plus history surgery — interactive rebase, cherry-pick, stash, conflict resolution and undo/recovery.

## Positioning

- An open-source alternative in the space GitKraken occupies.
- The commit graph is a major focus of the product, not one view among many.
- Tooling for the conventions around commits and repository documents is part of the client, not left to external CLIs and plugins.
- One interface serves beginners and experts: intuitive by default, with shortcuts and a command box on top.
- Built on a model that is not git-specific, so other version control systems can be added. Jujutsu (jj) support is planned once git works.

## Operating Context

- Used on a developer's desktop alongside an editor and a terminal, on local repositories.
- This repository itself follows the conventions VOGT wants to assist with: gitmoji + Conventional Commits (`docs/commit-conventions.md`), ADRs in `docs/adr/` and a root `GLOSSARY.md` (`docs/agents/domain.md`), and GitHub Issues as the tracker.

## Capabilities and Constraints

- **Stack:** SvelteKit SPA (Svelte 5 runes, strict TypeScript) inside a Tauri 2 shell. Everything that touches the system is a Rust Tauri command; there is no Node server at runtime. Tailwind CSS 4 and daisyUI 5 are installed.
- **Localisation:** an inlang project (`project.inlang/`, `messages/`) is set up; no messages exist yet.
- **Current state:** see **Project status** in [AGENTS.md](AGENTS.md).
- **Sequence:** git first; jj after git works; ADR tooling later.

Open decisions:

- Which forges are supported, and in what order.
- What "repo knowledge tooling" covers beyond ADRs.
- Whether release and task running (version bumps, tags, running project scripts) belongs to "Operations". Not selected so far.
- Which version control systems follow jj, if any.
- Licence: `LICENSE` is GPL v3, while `package.json` and `Cargo.toml` declare MIT.

## Brand Commitments

- **Name:** VOGT — Version & Operations GUI Tool.
- **App icon:** `icon.svg` / `icon.png` at the repository root, bundled from `src-tauri/icons/`.
- **Interface, in the owner's words:** "clean functional but unique".
- **Colour is the user's choice:** the daisyUI themes must be usable. The design is not fixed to a palette, so its identity cannot depend on specific colours and every surface must hold up under any daisyUI theme, light or dark.
- **Reference:** GitKraken, as inspiration for the product. VOGT is an alternative to it, not a copy.

## Evidence on Hand

- The app icon. No screenshots, logo lockup, website or marketing copy exist.
- No users, testimonials, benchmarks or performance figures exist. Future work must not invent them, including speed or size comparisons against other clients.

## Product Principles

1. **The graph is the centre.** History is what users come to see and act on; other work is reached from it.
2. **Two speeds, one interface.** Every action is discoverable for someone new to git and directly reachable by keyboard or command box for someone who is not. No separate beginner and expert modes.
3. **Take the drain out of the routine.** Where a convention exists — commit format, gitmoji, ADRs — the app carries it so the user does not have to remember it.
4. **Git first, not git only.** Concepts and names in the interface should survive the arrival of jj and other systems.
5. **Rewriting history must be recoverable.** History surgery ships together with undo and recovery, never ahead of it.
