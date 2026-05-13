# KB Web · 设计 brief

This brief is self-contained: anyone (or any agent) reading it should be able
to redesign the KB web UI without consulting other files. The goal is an
Apple-HIG-flavored interface that fits the product's actual use, not a generic
admin dashboard. Color palette is locked; everything else is open.

The current implementation lives in `web/src/` (Astro SSR + bun). It's
functional — every page works, all flows are wired — but visually it's a
terminal-inspired dump. The redesign target: feel like a careful tool the
single user reaches for daily, not a CRUD form.

---

## 1. What this product is

A **personal delta knowledge base**. Five entry types (Fact, ProblemSolution,
Lesson, Decision, Heuristic), each with a strict schema. Only knowledge a
generic LLM would answer **incorrectly without lookup** belongs in here —
LLM-known public knowledge is noise.

Three implications for design:

1. **Single user, single device most of the time.** Desktop-first. Phone is
   useful for "I'm away from my laptop and need to look something up" — read
   path only. Tablet not a real target.
2. **Daily-use tool.** Not a once-a-month review app. Speed of write +
   speed of recall both matter. Latency budget for any interaction: <100ms
   perceived response.
3. **Writing is gated.** The user is supposed to hesitate before writing. The
   UI should make the gate feel deliberate, not bureaucratic — surface the
   "do you actually need this?" question without being preachy.

The full design spec is at `docs/design-v2.2.html` if deeper background is
useful.

---

## 2. Color palette (LOCKED — do not change)

Used as CSS custom properties throughout the current build. Names and intent:

| Token              | Hex       | Intended role                                  |
| ------------------ | --------- | ---------------------------------------------- |
| `--bg`             | `#14110d` | Page background. Warm near-black.              |
| `--surface`        | `#1c1814` | Card / panel backgrounds.                      |
| `--surface-2`      | `#221d18` | Elevated surface (modals, nested panels).      |
| `--border`         | `#2c2620` | Subtle hairline.                               |
| `--border-strong`  | `#3d3530` | Active / focused borders.                      |
| `--text`           | `#e8e2d5` | Primary text. Warm parchment, not pure white.  |
| `--text-secondary` | `#a39988` | Body prose, supporting copy.                   |
| `--text-muted`     | `#6b6356` | Metadata, captions, timestamps.                |
| `--gold`           | `#d4a857` | Accent — headings, links, primary actions.     |
| `--gold-dim`       | `#9e7d3f` | Hover / depressed states of gold.              |
| `--terracotta`     | `#b87b59` | Code, technical identifiers, ProblemSolution.  |
| `--green`          | `#8aa84a` | "Verified / success" — Fact kind.              |
| `--vermilion`      | `#c87850` | "Warning / trap" — Lesson kind, dangers.       |
| `--lapis`          | `#6b95b0` | "Cool / methodology" — info accents.           |
| `--indigo`         | `#5b6a98` | "Cool / methodology" — Heuristic kind.         |

**Kind ↔ color mapping is semantic, not arbitrary** — keep it:

- Fact → green (it's been verified)
- ProblemSolution → terracotta (problem-shaped)
- Lesson → vermilion (caution — common mistake)
- Decision → gold (the user's deliberate choice)
- Heuristic → indigo (methodology / cool reasoning)

The overall palette read is **"old paper / scholarly notebook in a warm
study"**, not "VS Code dark theme." That distinction matters for typography
and surface treatment choices.

Dark mode only is fine — the user works in low-light environments and the
palette doesn't have a light counterpart.

---

## 3. Information architecture

Five top-level routes plus one transient state:

```
/                         List + filter + search
/entry/:ulid              Detail (read view)
/new                      Create entry
/edit/:ulid               Modify entry (kind immutable)
/stats                    Aggregate counts
/api/write → HTML         "Duplicates found" interstitial (see §5.4)
```

There is **no settings page, no account page, no help page** — single user,
all config is environment-side.

The skill bundle at `/skill/SKILL.md` is reachable via direct URL but is
documentation served as raw markdown — out of scope for this UI.

---

## 4. Data model summary

Every entry shares **common fields**:

```
id              ULID (sortable timestamp ID; never shown raw to humans except as a tag)
title           human-readable, mutable, not part of identity
body            free-form Markdown; may contain [[ULID]] inline references
tags            string array, convention path-prefix: "rust/tokio", "network/wireguard"
source          Human | ClaudeCode{session,project,cwd} | Imported{from,date}
created_at      monotonic
updated_at      bumped on update
version         incremented on update
deprecated_at   nullable; non-null = soft-deleted
superseded_by   nullable ULID of replacement
```

Plus **kind-specific fields**, all required unless marked optional. The ◆
marks indicate which fields participate in deduplication identity (the
"natural key"):

### Fact
- ◆ `claim` — one-sentence verified statement
- `evidence[]` — at least one of `{kind: Output|Url|SelfVerification, content, captured_at}`

### ProblemSolution
- `problem`
- `environment`
- ◆ `symptoms[]`
- ◆ `root_cause`
- `solution`
- `verification` (optional but encouraged)

### Lesson
- ◆ `trap` — the misleading thought "looks like X..."
- `correction` — what's actually true
- `why` — what makes the trap mislead
- `context` (optional) — when the trap arises

### Decision
- ◆ `context` — the situation
- ◆ `decision` — what was chosen
- `rationale`
- `alternatives[]` — `{option, why_not}`
- `tradeoffs[]`

### Heuristic
- ◆ `problem_class` — when to apply
- ◆ `pattern` — how to think about it (NOT step-by-step commands)
- `limits` (optional) — when it doesn't apply

### Lifecycle states

Just two: **active** (`deprecated_at IS NULL`) and **deprecated**. Default
queries exclude deprecated. Deprecation can be standalone (no successor) or
via supersede (one atomic write replaces another).

There are no Draft / Pending / Review states. Writing == publishing.

---

## 5. Pages in detail

### 5.1 List `/`

**Purpose**: scan or search what's in the KB. This is the most-used screen.

**Current content** (re-imagine the layout):
- Search input (FTS query — uses libsimple `simple_query`, wide recall)
- Filter chips: 5 kinds (toggleable, multi-select)
- Filter chip: "show deprecated" toggle
- Active tag filter (when set via clicking a tag elsewhere — shows with × to clear)
- Result list: each row has title, kind badge, updated date, summary (first 120 chars of body), tag chips

**What the user actually does here**:
- "Did I write that thing about WireGuard MTU last month?" → search + tag click
- "What have I been writing about lately?" → just scroll the default list (sorted updated DESC)
- "I want to see all Decisions." → kind filter

**Constraints**:
- The API returns max 50 results — no infinite scroll, no pagination needed.
- Results carry no body, only summary. Don't try to show full content here.
- Score from FTS is in the data when an FTS query is active (`hit.score` — negative bm25, smaller = more relevant). Currently invisible; might be useful to surface.

**Density target**: each entry should be scannable. Title is the primary
hook. Kind badge + updated date are secondary metadata. Tags and summary are
tertiary — only consult if title doesn't tell you what it is.

### 5.2 Detail `/entry/:ulid`

**Purpose**: read one entry fully. Sometimes the destination of a `[[ULID]]`
cross-reference.

**Current content**:
- Title (large)
- Kind chip, deprecated chip (if applicable), version, ULID, updated date, edit link
- Tag chips (each a link back to `/?tag=foo`)
- Deprecation callout (if deprecated, with link to `superseded_by` if any)
- **Structured field blocks** — one box per kind-specific field, with a
  monospace label and the value below. The ◆ marks NK fields.
- Body section — rendered Markdown. `[[ULID]]` inline references become small
  chip-style links to other entries.
- Lifecycle section — single text input + "Deprecate" button.

**Important**: structured fields are the **source of truth**. Body is for
prose that doesn't fit the schema. The hierarchy should reflect that — fields
prominent, body subordinate (or even collapsed-by-default for short ones).

**The `[[ULID]]` inline link is a small chip with terracotta-tinted
background** — visually distinct from regular Markdown links because they
mean "another KB entry." Click navigates within the KB.

### 5.3 New `/new` and Edit `/edit/:ulid`

**Purpose**: write or modify an entry. Shared layout because they're
basically the same form.

**Current content** (and pain points to fix):

- Kind selector (dropdown, full list of 5).
- Title, tags inputs.
- Type-specific fields — currently rendered as **5 collapsed `<details>`
  sections**, one per kind. User opens the matching one and fills it in. This
  is a bad UX: it shows fields you don't need and hides ones you do.
- Body textarea.
- Supersedes ULID (optional, on create only).
- Submit / Cancel.

**Critical UX problem on /new**: changing the kind should swap which fields
are visible. Currently the form shows all 5 kinds' fields at once. Tweak point
(see §8) — this is one of the biggest design decisions.

**On /edit**: kind is immutable (changing kind requires supersede). The
form should reflect that — kind shown as a static label, not an editable
control.

**Field hints** the form should make legible:
- ◆ marks an NK field (changes here affect dedup identity)
- `[[ULID]]` syntax inside body becomes a cross-reference
- Tags should follow `domain/specific` convention — autocomplete from existing
  tags would be a nice touch (data available via `/api/search` results' tags).

### 5.4 Duplicates resolution (post-write interstitial)

**This is the most important interaction in the whole product.** Currently
it's the ugliest part.

When the user submits `/new` and the server finds Layer 1 (exact hash) or
Layer 2 (FTS bm25) candidates, instead of returning JSON the server returns an
HTML page listing them. The user picks one of three paths:

1. **Update an existing entry** — "this is the same thing, I have more to say"
2. **Supersede** — "the old one was wrong, this replaces it"
3. **Force write as new** — "this is genuinely different despite the textual
   similarity"

For path 3, the form includes a server-issued `proceed_token` that must be
returned with `dedup=force`. The candidate list and the new entry's full
form data are carried back as hidden inputs so the user doesn't re-type.

**Design needs**:
- Candidates listed with kind, title, the natural-key text (so the user can
  see WHY each was flagged).
- Layer indicator: L1 hits ("exact hash match") look different from L2 hits
  ("FTS similar tokens").
- The three actions should be visually distinct and the chosen-most-often
  ones (probably "Force write" and "Update") most accessible.
- "Supersede X" needs to surface WHICH candidate is being superseded — best
  done by clicking on a candidate to enter supersede mode.

**Frequency**: this page appears maybe 1-in-5 writes. It's not rare. Make it
pleasant.

### 5.5 Stats `/stats`

Two tables:
1. Total / active / deprecated counts.
2. Per-kind breakdown (5 rows, active+deprecated per row).

Minimal. Could be a single panel on a sidebar instead of its own page — that's
a tweak point.

---

## 6. Critical interactions (specific to this product)

### KB-as-delta gate

Before submitting `/new`, the user is supposed to ask "could a generic LLM
answer this correctly without docs lookup?" If yes, don't write.

The current UI shows this as a paragraph of small italic text above the form.
That's weak. Tweak point: how to make this question feel like a real gate
without being annoying.

### Tags as path prefix navigation

Convention: `domain/specific`, like `rust/tokio`, `network/wireguard`,
`ops/headscale`. The KB filters by tag *prefix* (`tags LIKE '%network/%'`), so
clicking `network/wireguard` could optionally also offer to filter on
`network/` (broader). Two-level tag interaction is worth considering.

### `[[ULID]]` cross-references

These are inline in Markdown body. They render as chips in the detail view. A
user authoring an entry might want to:
- Paste an existing ULID and have the chip render live in a preview
- Search for an entry while writing and insert its ULID

Inline-reference UX during authoring is a tweak point — currently it's just
"type the ULID yourself."

### Search

Currently a single text input that triggers an FTS query. Two real use cases:
- "I remember some keywords" → text search (current)
- "Anything tagged `rust/`?" → tag prefix (current, but indirect — must click
  a tag chip somewhere first)

Tweak point: should there be a Cmd-K-style command palette that handles both
plus jump-to-ULID?

### Deprecation flow

Currently a text input + Deprecate button at the bottom of the detail page. A
deprecation is irreversible (the comment in code says "if you want to revive
it, write a new entry that supersedes the deprecated one"). The UI should
make this irreversibility legible — confirmation, or explicit "I understand"
language.

### Keyboard

Power-user single-tool warrants Cmd-K (search), Cmd-N (new), `/` (focus
search). Currently zero keyboard shortcuts. Tweak point.

---

## 7. Apple HIG application notes

These are direction-setting, not prescription. The designer should pick where
to lean.

**Typography**
- HIG's hierarchy via *weight* + *size*, not borders. Avoid the current habit
  of underlining everything with a 1px border.
- A sans like Inter / SF Pro / Geist will read closer to native than the
  generic `-apple-system` stack. JetBrains Mono / SF Mono / Berkeley Mono for
  monospace.
- Generous line-height (1.5–1.7 for body, ~1.3 for headings).
- Don't shrink body text below ~14–15px on desktop.

**Spacing**
- 8pt baseline grid.
- Reading-width caps: prose around 60–72ch, dashboards full-bleed within a
  reasonable max width (~1200px).
- Padding around cards, not borders, for separation. Or a hairline border
  with low contrast — but pick *one*.

**Surface elevation**
- In dark mode, elevation comes from *lighter* surface tone (not shadow).
  `--surface` / `--surface-2` are already arranged for this. Add a third if
  needed.

**Affordance & interaction**
- Hover states should be present but subtle (HIG-ish: slight tone shift, not
  color jump).
- Focus rings: a clean 2px outline in `--gold-dim` at 2px offset, like macOS
  Tahoe. Visible only on keyboard focus (`:focus-visible`).
- Click targets ≥ 32px (HIG mobile baseline) even on desktop.

**Navigation**
- macOS apps use sidebars. Web apps with sidebars at this size feel native.
  An alternative is a top horizontal bar (current). Sidebar would surface
  kinds + tag-prefix tree as standing affordances.

**Density**
- HIG's "comfortable" by default, with the option to go "compact." The list
  view could offer a density toggle (Mail-app style). Optional.

**Animation**
- Sparing, ease-out, ~150ms. Page transitions don't need anything. Filter
  toggles and modal open/close benefit from a quick fade.

**No iOS chrome metaphors** — this is desktop-first. Don't bring tab bars
or large titles in pretending to be iPad.

---

## 8. Tweak points (designer decisions)

Each is a real fork. Pick deliberately; don't default-think.

1. **Primary navigation**: top bar (current) vs. left sidebar (more macOS-y,
   surfaces kinds and tags as standing nav).
2. **List density**: comfortable vs. compact vs. user-toggle. Comfortable
   likely a better default for a daily tool.
3. **Search location**: top of list (current) vs. global Cmd-K palette vs.
   both. Cmd-K is a strong recommendation.
4. **Kind switching on /new**: dropdown (current) vs. segmented control vs.
   first-step picker that swaps the whole form. Pick one and commit — the
   five `<details>` panels approach is the worst.
5. **Field layout in /new and /edit**: single-column form vs. two-pane (live
   preview right) vs. inspector-style sidebar with metadata fields. Body and
   structured fields share weight differently in each.
6. **Body Markdown editor**: textarea (current) vs. CodeMirror/Monaco-lite
   vs. minimal contenteditable. `[[ULID]]` autocomplete only viable in 2 or 3.
7. **Tags input**: comma-separated text (current) vs. chip-based with
   autocomplete vs. two-level path picker. Chip-based with autocomplete is
   probably the right balance.
8. **Detail layout**: single-column long-form (current) vs. left
   structured-fields / right body vs. tabs across `fields | body | history`.
9. **Duplicates resolution**: dedicated page (current) vs. modal/sheet over
   /new vs. inline expansion below the form. Sheet is most HIG-flavored.
10. **Deprecate**: bottom-of-page form (current) vs. ⋯ menu in the page
    header vs. swipe-action in list view. Menu is the cleanest.
11. **Stats page existence**: own page (current) vs. footer summary on / vs.
    sidebar widget. Probably collapse into a `/`-page sidebar widget — it's
    too thin for a route.
12. **Keyboard shortcuts**: which to add. At minimum Cmd-K (search), Cmd-N
    (new), `j`/`k` (next/prev in list), `e` (edit on detail), `Cmd-Enter`
    (submit form).
13. **Empty states**: list with no results, no entries at all, search with
    zero hits. Currently a plain "no entries match" string. Could be an
    empty-state with a hint pointing at /new or at adjusting filters.
14. **Mobile / narrow viewports**: do we ship a real responsive layout, or
    just stack things and accept the read experience? My recommendation:
    stack-and-accept. The write flow is rare on phone.
15. **`[[ULID]]` chip rendering**: just text "↗ ULID" (current) vs. fetched
    on-render to show the linked entry's title vs. tooltip preview. Fetched
    title would be ideal but requires async render in detail.
16. **Score display on FTS results**: hide (current) vs. show as faint
    relevance bar / number. Probably hide — the score is internal.
17. **The KB-as-delta gate on /new**: small paragraph (current) vs. visible
    checkbox "yes, I confirmed an LLM wouldn't answer this" vs. simply trust
    the user. The checkbox feels paternalistic; trust the user but make the
    field hints (the "why this kind" copy) good enough to enforce
    self-gating.
18. **`source` field on /new**: currently always set to `Human` server-side.
    Worth surfacing? Probably no — `ClaudeCode` source will come from MCP
    writes, `Human` from web. No UI need.
19. **Deprecated-included visual treatment**: when "show deprecated" is on,
    how should deprecated entries look in the list? Strikethrough title?
    Lower opacity? A small chip? Choose one consistent treatment.
20. **Per-kind icon glyph**: do kinds get a single iconographic symbol (lucide
    or SF-symbol-style) in addition to color, or is color alone enough? Color
    alone risks accessibility issues; an icon helps even for the user's own
    quick scanning.

---

## 9. Out of scope (don't redesign these)

- **The API**. REST routes and the JSON shapes they return / accept are
  fixed. The MCP tool surface is fixed.
- **The five entry kinds**. The schema is intentional and validated server-
  side. The redesign can change how fields are *laid out*, not whether they
  exist.
- **The natural-key dedup pipeline**. Layer 1 + Layer 2 behavior is fixed.
  The UI can surface candidates differently, but the algorithm is locked.
- **Color tokens**. Names, hex values, and kind ↔ color mapping are fixed.
- **Real-time collaboration**. Single user.
- **i18n**. Single user (currently in Chinese context but text is mostly
  English-leaning in the codebase). Pick one language; don't build a
  framework around it.
- **Light mode**. Dark only.
- **Native mobile app**. Web only.

---

## 10. Files in the current implementation

For reference / before-and-after diffs:

```
web/
├── src/
│   ├── layouts/Base.astro       Top bar + skeleton
│   ├── components/
│   │   └── EntryCard.astro      One row in the list
│   ├── lib/
│   │   ├── kb-client.ts         Server-side fetch wrapper
│   │   ├── markdown.ts          marked + [[ULID]] post-process
│   │   ├── types.ts             TS types mirroring Rust schema
│   │   └── form.ts              FormData → WriteInput
│   ├── pages/
│   │   ├── index.astro          List + filter
│   │   ├── entry/[id].astro     Detail
│   │   ├── new.astro            Create
│   │   ├── edit/[id].astro      Modify
│   │   ├── stats.astro          Stats
│   │   └── api/                 server-side POST/PATCH endpoints
│   └── styles.css               All current CSS
└── astro.config.mjs
```

The Astro SSR setup is fine to keep — pages render server-side, no client-side
JS framework. Add interaction with vanilla JS / Web Components if needed
(htmx-style partials work cleanly with the existing /api/* endpoints).

---

## 11. Success criteria

After redesign, the user should be able to:

- Open `/`, find a specific known entry in under 5 seconds by typing one to
  three keywords.
- Create a new ProblemSolution in under 90 seconds (most of which is typing
  the actual content, not navigating the form).
- Read a detail page on a phone screen without horizontal scroll.
- Handle a duplicates-found response without re-reading the brief.
- Open the app and feel like it was designed *for them*, specifically, not
  scaffolded.

The current build clears 1, 2, and 3 functionally but fails 4 and 5
aesthetically. The redesign target is 5/5.
