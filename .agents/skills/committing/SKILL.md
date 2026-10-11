---
name: committing
description: Use when creating, amending, or rewording any git commit in this repository — picking the gitmoji, type and scope, writing the message, and getting the authorship right — a person in the loop is the author with the agent as Co-Authored-By, an unattended run authors as the agent, and an agent that only writes the message adds no trailer. Verify the result before pushing.
---

# Committing

## Overview

Every commit follows [`docs/commit-conventions.md`](../../../docs/commit-conventions.md) — a hybrid of Conventional Commits and gitmoji. This skill is the procedure for an agent; the guide is the reference for people.

Two rules are easy to get wrong, which is why the skill is mandatory:

1. The header is `<emoji> <type>(<scope>): <description>` — one gitmoji from the allowed table, one Conventional Commits type, a scope whenever one area applies.
2. **The authorship depends on the situation.** Work you did with a person: the person is the author and you are the co-author, `Co-Authored-By: <Platform> (<model-id>) <noreply@vendor>`. Work you did unattended: you are the author under that identity, no trailer. Work you only commit: the person is the author, no trailer — writing the message is not authorship. In doubt about your own work, use the co-author form (Step 4).

## Step 1 — Read the change

```sh
git status --short
git diff --cached
```

Stage exactly what the commit is about (`git add <paths>`, not `git add -A`). One commit, one change; if the description needs the word "and", split it. Note whether you made these changes — Step 4 depends on it.

## Step 2 — Pick emoji, type and scope

| Type       | Emoji | Use for                                | Allowed variants                                                                              |
| ---------- | ----- | -------------------------------------- | --------------------------------------------------------------------------------------------- |
| `feat`     | ✨    | a new capability                       | —                                                                                             |
| `fix`      | 🐛    | a defect fixed                         | 🚑️ production hotfix, 🔒️ security                                                             |
| `docs`     | 📝    | documentation                          | 💡 source comments                                                                            |
| `style`    | 💄    | formatting only, no behaviour change   | —                                                                                             |
| `refactor` | ♻️    | same behaviour, different code         | 🔥 delete code or files, 🚚 move or rename, ⚰️ remove dead code                                |
| `perf`     | ⚡️    | same behaviour, faster                 | —                                                                                             |
| `test`     | ✅    | tests                                  | —                                                                                             |
| `build`    | 📦️    | build system and dependencies          | ⬆️ upgrade a dependency, ⬇️ downgrade a dependency, ➕ add a dependency, ➖ remove a dependency |
| `ci`       | 👷    | CI configuration                       | 💚 fix CI                                                                                     |
| `chore`    | 🔧    | housekeeping that is none of the above | 🔖 release or tag, 🙈 .gitignore, 🔨 development scripts                                      |
| `revert`   | ⏪️    | undoing an earlier commit              | —                                                                                             |

Scope: the one area the change belongs to, in lowercase kebab-case — a module directory under `src/` (`ui`, `render`, `world`, `save`, `nav`, `behaviour`) or a subsystem's name (`assets`, `models`, `buildings`, `walls`, `tools`, `harness`, `e2e`, `ci`, `deps`, `docs`, `commits`). Omit it only for a repo-wide change.

## Step 3 — Write the message

```
<emoji> <type>(<scope>): <description>

Why and what changed, wrapped at 72 columns.
```

- Description: imperative, lowercase, no trailing period; the whole header is at most 72 characters.
- Body: optional, blank line before it; say what and why, not how.
- Breaking change: `!` before the colon and a `BREAKING CHANGE:` footer.
- `Co-Authored-By: <Platform> (<model-id>) <noreply@vendor>` — in work you did with a person this is your line: it is where your platform and model are disclosed. When you are the author (unattended), there is no trailer — the author identity already discloses them. When you only wrote the message, there is no trailer either.
- `Closes: #123` / `Refs: #123` when an issue is involved.
- Nothing else — no "Generated with …" boilerplate.

## Step 4 — Get the authorship right

**First: did you work on this change?**

**No — you were asked only to commit what is already here.** The person is the author and the message carries no trailer of yours. If the change is mechanical — formatting, a formatter's or linter's output, a dependency bump, generated files — it can be assumed to have no AI involvement, and you are done. Otherwise, tell the person that the code may need an AI-disclosure trailer and the commit may have to be amended; naming the agent that wrote the code is their call. Never invent a trailer for work you did not do.

**Yes — then:**

| Situation                                                                          | Author                                                                               | Message                          |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | -------------------------------- |
| A person is part of the work — an interactive session, work they asked for or shaped | the person, under the repository's configured `user.name`/`user.email` (pass nothing) | ends with your `Co-Authored-By`  |
| Unattended — scheduled task, CI, cloud run with nobody in the loop                  | you, via `-c user.name`/`-c user.email`                                              | no trailer                       |
| In doubt                                                                            | the person, as above                                                                 | your `Co-Authored-By`            |

Your identity — the co-author in the first case, the author in the second:

- **platform** — the runtime's name: `OpenCode`, `Claude Code`, `Codex CLI`, `Gemini CLI`, `Cursor`, …
- **model** — the model ID you are running as, exactly as your runtime reports it (e.g. `deepseek-v4.1-flash`, `claude-opus-4-5`, `gpt-5-codex`, `gemini-2.5-pro`).

| Platform    | Name                       | Email                             |
| ----------- | -------------------------- | --------------------------------- |
| OpenCode    | `OpenCode (<model-id>)`    | `noreply@opencode.ai`             |
| Claude Code | `Claude Code (<model-id>)` | `noreply@anthropic.com`           |
| Codex CLI   | `Codex CLI (<model-id>)`   | `noreply@openai.com`              |
| Gemini CLI  | `Gemini CLI (<model-id>)`  | `noreply@google.com`              |
| Cursor      | `Cursor (<model-id>)`      | `noreply@cursor.com`              |
| any other   | `<Platform> (<model-id>)`  | `noreply@<platform-slug>.invalid` |

If you cannot determine your model, stop and ask the user. Never omit it, never guess it, never write a placeholder. A change you made with a person, committed without a trace of you, is a bug; so is an unattended commit under the person's identity.

## Step 5 — Commit

Work you did with a person — the person is the author, you the co-author:

```sh
git commit -F - <<'EOF'
✨ feat(ui): add the settings screen

Why it exists and what changed, wrapped at 72 columns.

Co-Authored-By: OpenCode (deepseek-v4.1-flash) <noreply@opencode.ai>
EOF
```

The same command without the `Co-Authored-By` line is the commit-only case — you are only writing the message.

Work you did unattended — you are the author, no trailer:

```sh
git -c user.name='OpenCode (deepseek-v4.1-flash)' \
    -c user.email='noreply@opencode.ai' \
    commit -F - <<'EOF'
✨ feat(ui): add the settings screen

Why it exists and what changed, wrapped at 72 columns.
EOF
```

`-F -` reads the message from stdin, so the body needs no shell quoting. `-c` sets the identity for this one commit only; in the person-led case you pass nothing, so the repository config keeps the person as author.

## Step 6 — Verify

```sh
git log -1 --format='%s%n%n%an <%ae>%n%n%b'
```

- Work you did with a person: the author is the person, not you, and the message ends with your `Co-Authored-By` including the model.
- Work you did unattended: the author is `<Platform> (<model-id>) <noreply@vendor>` and there is no trailer.
- Commit-only: the author is the person, the message has no trailer of yours, and — unless the change was mechanical — you have raised the disclosure question.

If it is wrong, fix it before pushing — `git commit --amend -F -` keeps the author; pass the right `-c` flags when you are amending the identity itself.

## Rewriting history

Amending a commit keeps the author it already has. A person-led commit keeps the person as the author and your trailer; your own unattended commit keeps you as the author. Never change whom a commit credits. If the person decides a commit-only change needs an AI-disclosure trailer, amend it with the trailer they name — never with your own, and never with one you invented. Work a subagent contributed to is credited with the agent that runs the commit; one commit, one author.

## Red flags — stop

- adding your own `Co-Authored-By` when you only wrote the message
- committing code that may be AI-written without telling the person — in doubt, raise it
- a change you made with a person, with no `Co-Authored-By` trailer — in doubt, add it
- a change you made with a person, authored under your own identity or with `-c user.name`/`-c user.email` passed
- an unattended commit authored under the person's identity
- a `Co-Authored-By` without a model, or with a guessed/placeholder model
- "Generated with …" boilerplate
- a header without an allowed emoji/type, or no scope for a one-area change
- several unrelated changes in one commit
