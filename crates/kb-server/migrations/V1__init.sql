-- Initial schema for KB v2.2.
--
-- Design decisions baked in:
-- * `entries.rowid` (INTEGER PRIMARY KEY AUTOINCREMENT) is reused as
--   `entries_fts.rowid`. The user-facing ULID lives in a separate column.
-- * tags / source / type_data are JSON TEXT (v2.1: no relational tag table).
-- * natural_key_hash is BLOB (raw 32 bytes) for compact equality lookup.
-- * `superseded_by` stores the ULID string of the replacing entry.
-- * The FTS5 virtual table is contentless; original text lives in `entries`.

CREATE TABLE entries (
    rowid              INTEGER PRIMARY KEY AUTOINCREMENT,
    ulid               TEXT    NOT NULL UNIQUE,
    kind               TEXT    NOT NULL,
    title              TEXT    NOT NULL,
    body               TEXT    NOT NULL DEFAULT '',
    tags               TEXT    NOT NULL DEFAULT '[]',
    source             TEXT    NOT NULL,
    type_data          TEXT    NOT NULL,
    natural_key_text   TEXT    NOT NULL,
    natural_key_hash   BLOB    NOT NULL,
    created_at         TEXT    NOT NULL,
    updated_at         TEXT    NOT NULL,
    version            INTEGER NOT NULL DEFAULT 1,
    deprecated_at      TEXT,
    deprecation_reason TEXT,
    superseded_by      TEXT
);

CREATE INDEX idx_entries_hash_kind   ON entries(natural_key_hash, kind);
CREATE INDEX idx_entries_kind        ON entries(kind);
CREATE INDEX idx_entries_updated_at  ON entries(updated_at);
CREATE INDEX idx_entries_deprecated  ON entries(deprecated_at);
CREATE INDEX idx_entries_superseded  ON entries(superseded_by);

CREATE TABLE entry_history (
    history_id  INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_rowid INTEGER NOT NULL,
    version     INTEGER NOT NULL,
    snapshot    TEXT    NOT NULL,
    recorded_at TEXT    NOT NULL,
    FOREIGN KEY (entry_rowid) REFERENCES entries(rowid) ON DELETE CASCADE,
    UNIQUE (entry_rowid, version)
);

CREATE INDEX idx_history_recorded ON entry_history(recorded_at);

-- FTS5 with simple tokenizer (loaded as runtime extension).
-- column order is significant: bm25() column weights are positional.
--   col 0: natural_key_text
--   col 1: title
--   col 2: body
CREATE VIRTUAL TABLE entries_fts USING fts5(
    natural_key_text,
    title,
    body,
    tokenize = 'simple',
    content  = ''
);
