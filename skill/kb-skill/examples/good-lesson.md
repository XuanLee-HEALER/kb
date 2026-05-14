# Good Lesson

```json
{
  "kind": "Lesson",
  "title": "tokio::spawn returns a JoinHandle, not a future",
  "trap": "It looks like calling tokio::spawn on an async block returns a future you await — so people try to chain .await directly without realizing it's the JoinHandle that needs awaiting, leading to confusion about what gets cancelled when",
  "correction": "tokio::spawn returns a JoinHandle<T>. Awaiting the handle waits for completion, but DROPPING the handle does NOT cancel the spawned task — the task keeps running. To cancel, abort() on the handle or use a CancellationToken.",
  "why": "The handle's API is shaped like a future (it implements Future), so the muscle memory of 'this looks like a future, I drop it, it cancels' is wrong. Tasks survive their handles unlike, say, std::thread::JoinHandle's drop behavior.",
  "context": "Hits people coming from std::thread or smol",
  "tags": ["rust/tokio"]
}
```

**Why Lesson and not ProblemSolution:**

The frame is "what you'd *expect* vs. what's actually true." That's
Lesson-shaped. There's no specific environment or symptom — this is a
conceptual confusion.

**What makes the `why` field load-bearing:**

The fix is one line ("use abort() or CancellationToken"). The *value* is the
why — it explains what makes the trap mislead you, so you can spot the same
pattern (handle-shaped types that don't behave like their analogues) in the
future.
