# Bad Lesson — missing meaningful correction

```json
{
  "kind": "Lesson",
  "title": "Async stuff is tricky",
  "trap": "Working with async Rust is hard",
  "correction": "Be careful",
  "why": "Async is complicated",
  "tags": ["rust"]
}
```

**Why this is useless:**

- `trap` is too vague to *identify* a specific misleading pattern.
- `correction` doesn't actually correct anything.
- `why` doesn't explain the mechanism that makes the trap mislead.

A Lesson that doesn't give a future-you a concrete actionable rewrite is
just noise. If you can't fill `correction` and `why` with something
specific, the experience hasn't crystallized yet — sit with it.
