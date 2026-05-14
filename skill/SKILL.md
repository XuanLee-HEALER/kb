---
name: kb
description: |
  Personal KB for delta knowledge. Five entry kinds (Fact, ProblemSolution,
  Lesson, Decision, Heuristic) with strong schema and Layer-1/Layer-2 dedup.
  Optimized for "things a generic LLM doesn't already know correctly."
---

# KB · usage guide for Claude Code

This server is the user's **delta knowledge base** — only entries that capture
something a generic LLM wouldn't answer correctly should land here. Anything
public-and-obvious is noise.

## When to write

**Write** when one of these holds:

- You just solved a non-trivial problem and the diagnosis was non-obvious.
- You hit a counter-intuitive trap (you expected X, it was Y).
- You made an architectural choice with explicit tradeoffs.
- You distilled a thinking pattern for a class of problems.
- You verified a fact about *this user's environment* that's not in upstream docs.

**Do not write** when:

- The fact is public knowledge (e.g. "OAuth flow is X", "tokio is work-stealing").
- You're mid-investigation and the conclusion isn't validated yet.
- It's a one-off workaround you won't repeat.
- It's project-specific minutiae that's only relevant to a project you won't revisit.

## The KB-as-delta filter

Before every write, ask:

> Could a generic LLM, with **no documentation lookup**, answer this correctly?

- **Yes** → don't write.
- **No** → write it.

This single check is the entire quality floor of the KB. Be strict.

## Choosing a kind

Walk this checklist top-down:

- Verified objective claim? → **Fact**
- "It looks like X but actually Y"-shaped trap? → **Lesson**
- "In environment E, symptoms S have root cause R, fix is Sol"? → **ProblemSolution**
- Architectural choice with tradeoffs? → **Decision**
- "How to *think* about this class of problem"? → **Heuristic**
- Doesn't fit any of the above? → don't write. Sit with it.

### Common mis-classifications

- "Steps to deploy X" → **don't write at all**. That's a Playbook — versioned
  artifacts go stale. Look up the actual docs when needed.
- "X in env Y has solution Z" → **ProblemSolution**, never Lesson. Lessons are
  about *thinking* errors.
- "I prefer tokio because…" → **Decision**, not Heuristic. Decisions are points;
  heuristics are processes.

## Write & read mechanics

The *mechanics* — argument shape, return-status branches, error strings,
when to choose `update` vs `write+supersedes` vs `deprecate`, the two
`search` modes — are documented inside each MCP tool's description. Read
them there; that's the source of truth and it lives next to the schema.

This file is for the higher-level question: should you write at all, and
which kind. Once those are decided, the tool descriptions tell you how.

A v3 `kb.semantic_search` (embedding-based recall) is in the design doc but
not yet implemented; today the only retrieval paths are `kb.search` (struct
+ FTS) and `kb.recent`.

## Tag conventions

Path-prefix style: `domain/specific`. Examples: `network/wireguard`,
`rust/tokio`, `ops/headscale`, `macos/hypervisor`. The KB doesn't enforce
this — it's a convention so `tags LIKE '%network/%'` works as a cheap facet.

Don't go more than two levels deep. `rust/tokio/scheduler` is overkill.

## Anti-patterns

- **Body duplicates structured fields.** Body is for prose that doesn't fit in
  the schema. If it's just a re-statement of `claim` or `root_cause`, drop it.
- **Lesson with a `correction` that reads like a playbook.** Lesson is about
  the *thinking error*. The fix belongs in `correction` as one sentence; if
  it's longer, you probably want a ProblemSolution.
- **Fact with `evidence` = "I remember reading this".** No. Either you have an
  output / URL / your own verification trace, or it's not a Fact yet.
- **Heuristic with `pattern` = "run these commands…".** That's a playbook.
  Heuristics describe how to *think*, not how to *execute*.
- **Decisions chained as updates instead of supersede.** If the decision
  changed, write a new entry that `supersedes` the old one. The supersede chain
  is your ADR evolution history.
- **Writing while still uncertain.** "I think" + commit = noise. Wait until
  you've verified or until the choice has been made.

## Examples

See `examples/` directory in this Skill bundle for good and bad cases of each
kind.

## Connection

Server endpoint is configured in `mcp.json`. The token is provisioned per
user — ask the human if you don't have one. The KB is single-user; sharing the
token is sharing the KB.
