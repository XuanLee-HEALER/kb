-- Raw candidate pool. Hook-only inbox for "session is about to compact/exit,
-- here is unrefined content that might be worth keeping". Every operation
-- (promote / discard) physically removes the row — the table only ever holds
-- pending candidates. Decision history is not persisted (would drown signal).

CREATE TABLE candidates (
    ulid       TEXT NOT NULL PRIMARY KEY,
    content    TEXT NOT NULL,
    source     TEXT NOT NULL,       -- JSON of kb_core::Source
    created_at TEXT NOT NULL
);

CREATE INDEX idx_candidates_created_at ON candidates(created_at DESC);
