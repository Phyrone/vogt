# Commit Conventions

Every commit in this repository — by a person or by an agent — uses the one format below. It is a hybrid of [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) and [gitmoji](https://github.com/carloscuesta/gitmoji): the Conventional Commit fields keep the history parseable, the gitmoji says at a glance what kind of change a commit is.

```
<emoji> <type>(<scope>): <description>

<body>

<footer>
```

The skill for agents is [`.agents/skills/committing/`](../.agents/skills/committing/SKILL.md); it repeats this format, the authorship rules and the verification step.

## The header

One line:

```
✨ feat(buildings): gatehouses let units through
🐛 fix(ui): keep the context menu inside the window
♻️ refactor(save): group scene objects by type
```

- **emoji** — exactly one, from the table below. It is the gitmoji for what happened.
- **type** — exactly one, from the table below.
- **scope** — the one area the change belongs to, in lowercase kebab-case: a module directory under `src/` (`ui`, `render`, `world`, `save`, `nav`, `behaviour`) or a subsystem's name (`assets`, `models`, `buildings`, `walls`, `tools`, `harness`, `e2e`, `ci`, `deps`, `docs`, `commits`). Write it whenever one area applies. Omit it only for a change that spans the repository, such as a toolchain upgrade or CI-wide housekeeping.
- **description** — imperative mood (`add`, not `added` or `adds`), lowercase, no trailing period. The whole header is at most 72 characters and must never exceed 100.
- **breaking change** — put `!` right before the colon (`✨ feat(save)!: ...`) and explain in a `BREAKING CHANGE:` footer.

One commit carries one change. If the description needs the word "and", it is probably two commits.

## Types and emoji

| Type       | Emoji | Use for                                | Allowed variants                                                                                |
| ---------- | ----- | -------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `feat`     | ✨    | a new capability                       | —                                                                                               |
| `fix`      | 🐛    | a defect fixed                         | 🚑️ production hotfix, 🔒️ security                                                               |
| `docs`     | 📝    | documentation                          | 💡 source comments                                                                              |
| `style`    | 💄    | formatting only, no behaviour change   | —                                                                                               |
| `refactor` | ♻️    | same behaviour, different code         | 🔥 delete code or files, 🚚 move or rename, ⚰️ remove dead code                                 |
| `perf`     | ⚡️    | same behaviour, faster                 | —                                                                                               |
| `test`     | ✅    | tests                                  | —                                                                                               |
| `build`    | 📦️    | build system and dependencies          | ⬆️ upgrade a dependency, ⬇️ downgrade a dependency, ➕ add a dependency, ➖ remove a dependency |
| `ci`       | 👷    | CI configuration                       | 💚 fix CI                                                                                       |
| `chore`    | 🔧    | housekeeping that is none of the above | 🔖 release or tag, 🙈 .gitignore, 🔨 development scripts                                        |
| `revert`   | ⏪️    | undoing an earlier commit              | —                                                                                               |

Use the default emoji of the type unless a variant names exactly what happened. No emoji outside this table.

## The body

Optional, separated from the header by one blank line and wrapped at 72 columns. It says **what** changed and **why** — the diff already says how. If a reader would ask "why?", the body answers. Bullets with `-` are fine.

## The footer (trailers)

The last block of the message, separated by one blank line:

- `BREAKING CHANGE: <what breaks and how to migrate>` — required whenever the header has `!`.
- `Closes: #123` or `Refs: #123` — the issue a commit closes or informs.
- `Co-Authored-By:` — the agent, when it worked on the change and a person was part of the work. The person is responsible for the change and is the author; the agent is credited here as the co-author. A commit made unattended — a scheduled, CI or cloud run with no person in the loop — is authored by the agent itself and has no trailer. Writing the commit message is not authorship: an agent asked to commit changes it did not make adds no trailer of its own.
- Nothing else — no "Generated with …" boilerplate; the author and co-author already say who did what.

## Example

```
✨ feat(walls): stairs portal from the ground onto the walk

A player kept getting stuck at the foot of a stair because the
ribbon's ground obstacle still covered its first tread.

- The deck's first cell now welds to the ground layer.
- Steep stretches stay out of the walkable set.

Closes: #412
```

## Writing a commit by hand

Nothing to install. Stage what the commit is about, then:

```sh
git commit -F - <<'EOF'
📝 docs(ui): explain the settings screen

The section list now says where each setting is read from, which
was only in the plan document before.
EOF
```

If an agent wrote part of the change, add its trailer — `Co-Authored-By: <Platform> (<model-id>) <noreply@vendor>` — because an agent is never an uncredited contributor. If you are unsure what the message should say, read the staged diff (`git diff --cached`) before writing it. Check the result with `git log -1` (or `git log -1 --format='%s'`).

## Commits made by an agent

Who commits how depends on whether the agent worked on the change and whether a person was part of it. The person is responsible for the result, however much of the code the agent wrote; the agent is the one who carried it out.

### A person was part of the work

The person is the author. Commit under the repository's configured `user.name`/`user.email` — nothing special to pass — and end the message with the agent as the co-author:

```
Co-Authored-By: <Platform> (<model-id>) <noreply@vendor>
```

| Platform    | Co-author example                | Email                             |
| ----------- | -------------------------------- | --------------------------------- |
| OpenCode    | `OpenCode (deepseek-v4.1-flash)` | `noreply@opencode.ai`             |
| Claude Code | `Claude Code (claude-opus-4-5)`  | `noreply@anthropic.com`           |
| Codex CLI   | `Codex CLI (gpt-5-codex)`        | `noreply@openai.com`              |
| Gemini CLI  | `Gemini CLI (gemini-2.5-pro)`    | `noreply@google.com`              |
| Cursor      | `Cursor (claude-sonnet-4-5)`     | `noreply@cursor.com`              |
| any other   | `<Platform> (<model-id>)`        | `noreply@<platform-slug>.invalid` |

The model ID is the one the runtime itself reports — an ID such as `deepseek-v4.1-flash`, not a marketing name. Platform and model are supplied on every agent trailer; a placeholder or an omitted model is a bug. An agent that cannot establish its model stops and asks instead of guessing.

```sh
git commit -F - <<'EOF'
✨ feat(nav): keep the path through a gatehouse

Co-Authored-By: OpenCode (deepseek-v4.1-flash) <noreply@opencode.ai>
EOF
```

An interactive session counts, and so does any change the person asked for or shaped.

### No person was part of the work

The agent is the author — a scheduled task, CI, a cloud run with nobody in the loop. Commit under its own identity and leave the trailer out; the author discloses the agent and the model:

```sh
git -c user.name='OpenCode (deepseek-v4.1-flash)' \
    -c user.email='noreply@opencode.ai' \
    commit -F - <<'EOF'
✨ feat(nav): keep the path through a gatehouse
EOF
```

### You are only committing the work

The person asked the agent to commit changes it did not make. Writing the commit message is not authorship: the person is the author and no trailer of the agent's belongs in the message.

If an agent wrote part of that code — in an earlier session, or another agent — the person has to disclose it. That cannot be read from the diff alone, so:

- Changes that are mechanical — formatting, a formatter's or linter's output, a dependency bump, generated files — can be assumed to have no AI involvement.
- For anything else, say that the code may need an AI-disclosure trailer so the person can decide whether the commit has to be amended. Naming the agent that wrote the code is their call; the committing agent invents nothing.

### In doubt

When the agent cannot tell whether it worked on the change, treat it as work it did with the person: the person is the author and the agent the co-author. That is the safe default — an agent must never commit a change it made with no trace of itself, and must never author a commit under the person's identity when no person was involved. Doubt about _someone else's_ AI involvement is different: raise it with the person instead of adding a trailer.

One commit has one author. When the person is the author, the agent that runs the commit is the sole co-author; work a subagent contributed to is not credited separately.

When rewriting history, amending a commit keeps the author it has. A person-led commit keeps the person as the author and the agent's trailer; an unattended commit keeps the agent as the author. Never change whom a commit credits.

The `committing` skill must be used before running `git commit`.

## Merge and revert commits

- A merge commit keeps git's generated message (`Merge branch 'feat/walls'`); the commits it merges follow the format.
- A revert uses the `revert` type and names what is undone — `⏪️ revert(save): undo "group scene objects by type"` — with `Refs:` to the original commit. `git revert`'s generated message is acceptable if left unedited.

## Common mistakes

| Mistake                                                           | Fix                                                                   |
| ----------------------------------------------------------------- | --------------------------------------------------------------------- |
| `fixed stuff`                                                     | `🐛 fix(ui): keep the context menu inside the window`                 |
| `✨ Update readme.`                                               | `📝 docs: update the readme` — right type, imperative, no period      |
| `✨ feat(ui): add the button and fix the crash and bump deps`     | split into three commits                                              |
| `✨ feat: make the save format better`                            | add the scope: `✨ feat(save): ...`                                   |
| a change the agent made with a person, under the agent's identity | the person is the author; the agent belongs in `Co-Authored-By`       |
| a change the agent made with a person, no `Co-Authored-By`        | add the agent's trailer — `<Platform> (<model-id>) <noreply@vendor>`  |
| an unattended run committed under the person's identity           | commit as the agent: `-c user.name=... -c user.email=...`             |
| an agent that only wrote the message adding its own trailer       | no trailer; the person discloses any AI-written code                  |
| committing code that may be AI-written without telling the person | flag it — mechanical changes aside, the person decides the disclosure |
| an agent `Co-Authored-By` without a model                         | `<Platform> (<model-id>)` — both, every trailer                       |
