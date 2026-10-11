---
version: 1
slug: "src-routes-page-svelte"
primary_target: "src/routes/+page.svelte"
related_targets: []
---

# Main repository window

Scope: `src/routes/+page.svelte`, the window a repository opens into. Visitor mode: Operate.

Audience and job: a developer, fluent in git or not, checking where their branch stands and making the next commit. Content at this stage is a synthetic repository in `src/lib/repo/mock.ts`; no Tauri command is wired.

Constraints: every colour comes from a daisyUI theme role, so the surface must hold under any daisyUI theme; the default Tauri window can be as small as 800 by 600.

## Direction contract

THESIS: The three-pane git client everyone can already operate, with the commit graph redrawn as a rail diagram. It refuses the category's thin, same-weight graph lines with avatar dots, where branches are told apart only by hue.

OWN-WORLD: Branches are lines with a lettered badge. Thick 6px strokes, 45° bends only, hollow station rings for commits, a capsule for a merge, a ringed station for HEAD, dashes for uncommitted work. Line colours are theme roles in a fixed order (primary, secondary, accent, warning, info, success). Type is Barlow, Barlow Semi Condensed for badges and small caps labels, JetBrains Mono for hashes and counts only. Controls are stock daisyUI.

STORY: The user sees which line they are on and how it relates to the others, selects a commit or the working changes, and commits with type, scope and subject assembled for them.

FIRST VIEWPORT: Toolbar across the top (repository, current branch with badge, undo/redo, pull/push, branch/stash/pop, command box, theme). Left, a branch sidebar where every branch carries its badge. Centre, the commit table with the line map as its first column, working changes as the first row. Right, staged files and the commit composer; the commit button, in the current line's colour, is the primary action at the bottom right.

SIGNATURE INTERACTION: Choosing a branch in the sidebar dims every other line and its rows.

FORM: Standard shell with a vertical line map, chosen by the user in round two after steering away from the roll; the graph comes from grounded candidate 3 (transit diagram). Seed key 51a0bea9.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- How badge letters are derived for real branches, and what happens on collisions.
- Whether the horizontal strip returns as an optional view (the user liked it in round one).
- Which toolbar actions stay visible in a narrow window.
