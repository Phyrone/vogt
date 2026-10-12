---
name: writing-paraglide-messages
description: Use when writing or editing Paraglide JS / inlang message files (messages/{locale}.json) — nested message objects, simple and multi-variant messages, placeholders, markup tags, options, attributes, escaping, plurals and formatting. Self-contained file-format reference; no web fetch needed.
---

# Paraglide message files

## Overview

Messages live in `messages/{locale}.json` — one JSON file per locale, using `@inlang/plugin-message-format`. Everything below documents that file format; do not fetch the web for it. If a project pins a different plugin version, the [source docs](#canonical-sources) are canonical.

Files are plain JSON objects. **This project groups messages as nested objects.** Each value is either a simple message (a string) or a multi-variant message (an array wrapper). Paraglide compiles every message to a function on `m`.

```json
{
	"$schema": "https://inlang.com/schema/inlang-message-format",
	"app_name": "VOGT",
	"history": {
		"title": "History",
		"empty": "No commits yet"
	},
	"cta": "{#link to=|/docs|}{#b}Read docs{/b}{/link}"
}
```

- `$schema` is optional; it only gives editors autocomplete.
- `pathPattern` (`./messages/{locale}.json`) and `sort` live in `project.inlang/settings.json`, not here. `sort: "asc"` keeps diffs consistent.
- Keep every locale file in sync — all keys, same nesting.

## Nested objects and keys

Nested objects are supported (plugin v4+, SDK v2+). The plugin **flattens them with dot notation on import and unflattens them on export**, so nesting is purely how you organize the file.

- `history.title` above is called as `m["history.title"]()`, not `m.history.title()`.
- Root-level keys are plain: `m.app_name()`.
- **Max nesting depth is 5.** Deeper groups must be written as dotted key names: `"a.b.c.d.e.f"`.
- Prefer literal bracket access (`m["history.title"]`); a computed key (`m[key]`) hides usages from tooling.

## Message strings — placeholders and markup

Any string value may contain placeholders and markup. **Markup is `{#tag}…{/tag}`, never HTML/XML:** `{#b}` is correct, `<b>` is invalid and will not compile. Tag names are arbitrary, app-defined identifiers — Paraglide maps nothing to HTML for you.

| Write this | Meaning | What your code gets |
| --- | --- | --- |
| `{name}` | input placeholder | `m.x({ name })` |
| `{value: number style=percent}` | formatted placeholder | formatted input (space after `:`) |
| `{#tag}…{/tag}` | wrapping tag | renderer `tag`, receives `children` |
| `{#tag/}` | standalone tag, no children | renderer `tag`, no `children` |
| `{#tag key=\|literal\|}` | literal option | `options.key` |
| `{#tag key=$variable}` | option from an input | `options.key` |
| `{#tag @flag}` | boolean attribute | `attributes.flag === true` |
| `{#tag @variant=\|hero\|}` | literal attribute | `attributes.variant === "hero"` |

Use **options** for data a renderer needs (`to`, `rel`, an icon name) and **attributes** for flags (`@track`, `@variant`). A dynamic link target is an option, not interpolation: `{#link to=$url}`, then `m.x({ url: "/docs" })`.

### Escaping

`{` and `}` open placeholders. Escape literal ones with a backslash — doubled, because this is JSON:

```json
{
	"json_hint": "Objects look like \\{\"key\": \"value\"\\}",
	"path_hint": "Windows paths use \\\\ as a separator"
}
```

Inside a `|literal|`, escape `|` as `\|` and `\` as `\\`.

## Multi-variant messages

A message that varies by input — plural, gender, platform, exact number — is an **array** whose object holds `declarations`, `selectors` and `match`:

```json
{
	"basket": [
		{
			"declarations": ["input count", "local countPlural = count: plural"],
			"selectors": ["countPlural"],
			"match": {
				"countPlural=one": "{count} item",
				"countPlural=other": "{count} items"
			}
		}
	],
	"jojo": [
		{
			"declarations": ["input platform", "input userGender"],
			"selectors": ["platform", "userGender"],
			"match": {
				"platform=android, userGender=male": "{username} downloads it from Google Play.",
				"platform=ios, userGender=female": "{username} downloads it from the App Store.",
				"platform=*, userGender=*": "The app is available for every platform."
			}
		}
	]
}
```

- The **array wrapper is required**. An object with `match`/`selectors` but no array is parsed as nested simple messages — a silent, classic mistake.
- `declarations` define inputs and derived `local`s; `selectors` name what to match on; `match` keys pair `selector=value`, comma-separated, with `*` as the wildcard fallback. A raw input can be a selector directly.
- `plural` derives a category with `Intl.PluralRules`; add `type=ordinal` for 1st/2nd/3rd. For an exact number (ICU `=0`), alias the raw input (`local countPluralExact = count`) and put that selector **first** so it wins over a category.
- Built-in formatters: `plural`, `number`, `datetime`, `relativetime`, `date`, `time`, `icu:pound`. Pass **raw values** (`number`, `Date`) and let Paraglide format per locale.
- ICU MessageFormat 1 catalogs are imported by `@inlang/plugin-icu1`, which rewrites `{count, plural, …}` into this array shape; authoring new messages in this file means writing the array shape.

## Rendering — `message()` strips markup

`m.x()` always returns a **plain string** with markup wrappers stripped. To render, give Paraglide a renderer/snippet per tag name:

```svelte
<script lang="ts">
	import { ParaglideMessage } from "@inlang/paraglide-js-svelte";
	import { m } from "./paraglide/messages.js";
</script>

<ParaglideMessage message={m.cta} inputs={{}}>
	{#snippet link({ children, options })}
		<a href={options.to}>{@render children?.()}</a>
	{/snippet}
</ParaglideMessage>
```

React/Vue/Solid take a `markup` prop instead. For custom rendering, `m.x.parts()` returns `text`, `markup-start`, `markup-end` and `markup-standalone` parts carrying `name`, `options` and `attributes`. Missing wrapping renderer: children still render. Missing standalone renderer: nothing renders.

## Common mistakes

- Writing `<b>`/`<a href="…">` — use `{#b}`/`{#link to=|…|}`.
- "Correcting" `{#b}…{/b}` to HTML because it resembles Svelte syntax. It is the plugin's markup.
- Calling a nested message as `m.history.title()` — dotted keys use `m["history.title"]()`.
- Writing a variant message as a bare object without the array wrapper.
- Assuming `{#link}` becomes `<a>` automatically — you provide the renderer.
- Interpolating a URL (`href={url}`) instead of an option (`to=$url`).
- Expecting `m.x()` to return HTML — it strips markup; use the component or `parts()`.
- Passing pre-formatted strings to `number`/`datetime` instead of raw values.
- Reusing one key for unrelated UI that only happens to read the same today.

## Red flags — stop and check

- a message containing `<` or `>` as a tag
- a `match` object not wrapped in an array
- a nested key called with dots in the property chain (`m.a.b()`)
- a renderer you never defined in the message's own tag names
- "this looks like Svelte, so it must be wrong" — it is the message syntax

## Canonical sources

Only if the project's plugin version differs: `https://paraglidejs.com/markup`, `https://paraglidejs.com/formatting`, and the plugin README at `https://inlang.com/m/reootnfj/plugin-inlang-messageFormat`.
