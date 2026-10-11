---
name: VOGT
description: A git client whose commit graph is drawn as a line map, coloured entirely by the active daisyUI theme.
colors:
  surface: 'var(--color-base-100)'
  surface-chrome: 'var(--color-base-200)'
  rule: 'var(--color-base-300)'
  ink: 'var(--color-base-content)'
  ink-muted: 'color-mix(in oklab, var(--color-base-content) 70%, transparent)'
  line-1: 'var(--color-primary)'
  line-2: 'var(--color-secondary)'
  line-3: 'var(--color-accent)'
  line-4: 'var(--color-warning)'
  line-5: 'var(--color-info)'
  line-6: 'var(--color-success)'
  added: 'var(--color-success)'
  removed: 'var(--color-error)'
  emphasis: 'var(--color-neutral)'
typography:
  body:
    fontFamily: 'Barlow, ui-sans-serif, system-ui, sans-serif'
    fontSize: '0.875rem'
    fontWeight: 400
    lineHeight: 1.43
  body-strong:
    fontFamily: 'Barlow, ui-sans-serif, system-ui, sans-serif'
    fontSize: '0.875rem'
    fontWeight: 600
  heading:
    fontFamily: 'Barlow Semi Condensed, Barlow, ui-sans-serif, sans-serif'
    fontSize: '1.25rem'
    fontWeight: 600
    lineHeight: 1.375
  caps:
    fontFamily: 'Barlow Semi Condensed, Barlow, ui-sans-serif, sans-serif'
    fontSize: '0.75rem'
    fontWeight: 600
    letterSpacing: '0.07em'
  signage:
    fontFamily: 'Barlow Semi Condensed, Barlow, ui-sans-serif, sans-serif'
    fontSize: '0.875rem'
    fontWeight: 700
  data:
    fontFamily: 'JetBrains Mono Variable, ui-monospace, monospace'
    fontSize: '0.75rem'
    fontWeight: 400
rounded:
  badge: '5px'
  control: 'var(--radius-field)'
spacing:
  row: '36px'
  lane: '36px'
  graph-inset: '20px'
  pane: '16px'
components:
  line-badge:
    backgroundColor: '{colors.line-1}'
    typography: '{typography.signage}'
    rounded: '{rounded.badge}'
    size: '22px'
  commit-row:
    height: '{spacing.row}'
    typography: '{typography.body}'
  commit-row-selected:
    backgroundColor: 'color-mix(in oklab, var(--line) 12%, transparent)'
    typography: '{typography.body-strong}'
  commit-button:
    backgroundColor: 'var(--line)'
    textColor: 'var(--line-content)'
    typography: '{typography.signage}'
---

## Overview

VOGT is the three-pane git client people can already operate, with one thing redrawn: the commit graph is a rail diagram. Branches are **lines**, each with a lettered badge; commits are stations; merges are interchanges. Everything else is deliberately standard so that the graph is the only thing a newcomer has to learn.

The interface owns no colours. Every colour is a daisyUI theme role, so any daisyUI theme the user picks is a valid VOGT. Identity therefore lives in geometry and type: stroke weight, bend angle, station shapes, badges and the signage typeface.

## Colors

- Surfaces are `base-100` (work areas) and `base-200` (toolbar and sidebar), separated by 1px `base-300` rules. No shadows separate panes.
- Lines take theme roles in a fixed order: `primary`, `secondary`, `accent`, `warning`, `info`, `success`. A line's colour is set once as `--line` / `--line-content` (see `lineStyle` in `src/lib/repo/types.ts`) and reused by its badge, its strokes, its selected-row tint and, for the checked-out line, the commit button.
- `error` is never a line colour; it is reserved for removed lines and over-limit counters. `success` doubles as added-line counts.
- `neutral` marks emphasis that is not about a line: the HERE marker, checked type and scope chips, and the push button when there is something to push.
- Secondary text is `base-content` at 70% opacity, never a fixed grey.
- Never use a fixed Tailwind colour (`red-500`, `gray-700`) or a `dark:` variant.

## Typography

- **Barlow** for everything read as text, at 14px.
- **Barlow Semi Condensed** wherever the interface behaves like a sign: line badges, ref markers, small-caps group labels (`caps` utility) and the commit detail heading (`signage` utility).
- **JetBrains Mono** only for hashes, counts and the assembled commit header. It is never used to make something look technical.
- Fonts are self-hosted through `@fontsource`; the app runs offline.

## Layout

- The toolbar is also the title bar. On macOS the system's traffic lights are overlaid on its left and the toolbar leaves room for them; on Windows and Linux the window has no decorations and `WindowControls.svelte` draws minimize, maximize and close at its right end. Empty toolbar space drags the window.
- Toolbar on top, then three panes: branch sidebar, history, and a context pane that shows the commit composer for working changes or the detail of the selected commit.
- The history table and the graph share one grid: rows are 36px and lanes are 36px apart, which is what makes every bend 45°. Change both together (`PITCH` in `src/lib/graph/layout.ts`).
- Narrow windows (the minimum is 800×560): the sidebar collapses to a rail of line badges below 1024px, and the history table drops its author, hash and when columns by container width.

## Elevation & Depth

Flat. Panes are divided by rules, selection by a tint of the row's line colour. The only depth is whatever daisyUI gives its own controls in the active theme.

## Shapes

- Line strokes are 6px with round caps and joins; uncommitted work is a dotted stroke.
- A commit is a hollow ring filled with the surface colour. HEAD is a larger ring in `base-content` with a dot in the line colour. A merge is a capsule in `base-content`. Uncommitted work is a dashed ring.
- Line badges are 22px squares with a fixed 5px radius. Controls take their radius from the theme.

## Components

- **Line badge** (`LineBadge.svelte`): the letter of a line on its colour. It appears wherever a branch is named.
- **Commit graph** (`CommitGraph.svelte`): draws a `GraphLayout`; it knows nothing about git. Layout rules live in `layoutGraph`: the trunk keeps lane 0, every other line takes the innermost free lane, and the outer line is the one that bends.
- **History** (`History.svelte`): the commit table. Rows are buttons; arrow keys and j/k move the selection.
- **Branch sidebar** (`RefSidebar.svelte`): choosing a branch dims every other line and its rows. This is the signature interaction.
- **Commit composer** (`CommitComposer.svelte`): staging, type and scope chips, subject with a 50-character counter, and the assembled gitmoji + Conventional Commits header.
- Buttons, inputs, selects, checkboxes, badges and menus are stock daisyUI components.
- Icons are Lucide (`@lucide/svelte`), imported one by one from `@lucide/svelte/icons/<name>` at 16px with the default stroke. Toolbar actions keep a text label where there is room and fall back to the icon with a tooltip; an icon never replaces a line badge.

## Do's and Don'ts

- Do give anything that belongs to a branch that branch's `--line` colour.
- Do keep bends at 45° and strokes at one weight.
- Do check a new surface in at least one light, one dark and one unusual theme (retro, synthwave).
- Don't thin the lines or replace stations with avatars; that is the graph this design replaces.
- Don't add a colour that is not a theme role, and don't rely on two roles being distinguishable: some themes make them close, so badges carry a letter as well.
- Don't add a second display typeface or decorative rail-map ornament; the graph is the only place the metaphor is drawn.
