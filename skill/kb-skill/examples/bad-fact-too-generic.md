# Bad Fact — too generic

```json
{
  "kind": "Fact",
  "title": "OAuth 2.0 uses bearer tokens",
  "claim": "OAuth 2.0 protocol uses bearer tokens in the Authorization header",
  "evidence": [...]
}
```

**Why this is bad:**

Every LLM knows OAuth 2.0 uses bearer tokens. The KB-as-delta filter says
"could a generic LLM answer this correctly without lookup?" — yes, so this
shouldn't be in the KB.

**What might be worth writing instead:**

- "Our Headscale instance rejects OAuth tokens issued by Google Workspace for
  external users because of CO-XXX policy" — that's a Fact specific to *your*
  environment that no LLM would know.
- "OAuth refresh tokens on Linear's API silently expire at 90 days even though
  the docs say 'long-lived'" — a Fact that contradicts published docs (assuming
  you verified).
