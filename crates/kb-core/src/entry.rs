use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::source::Source;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum EntryKind {
    Fact,
    ProblemSolution,
    Lesson,
    Decision,
    Heuristic,
}

impl EntryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "Fact",
            Self::ProblemSolution => "ProblemSolution",
            Self::Lesson => "Lesson",
            Self::Decision => "Decision",
            Self::Heuristic => "Heuristic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "kind")]
pub enum KindData {
    Fact {
        claim: String,
        evidence: Vec<Evidence>,
    },
    ProblemSolution {
        problem: String,
        environment: String,
        symptoms: Vec<String>,
        root_cause: String,
        solution: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verification: Option<String>,
    },
    Lesson {
        trap: String,
        correction: String,
        why: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    Decision {
        context: String,
        decision: String,
        rationale: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        alternatives: Vec<Alternative>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tradeoffs: Vec<String>,
    },
    Heuristic {
        problem_class: String,
        pattern: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limits: Option<String>,
    },
}

impl KindData {
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        match self {
            Self::Fact { .. } => EntryKind::Fact,
            Self::ProblemSolution { .. } => EntryKind::ProblemSolution,
            Self::Lesson { .. } => EntryKind::Lesson,
            Self::Decision { .. } => EntryKind::Decision,
            Self::Heuristic { .. } => EntryKind::Heuristic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub content: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum EvidenceKind {
    Output,
    Url,
    SelfVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Alternative {
    pub option: String,
    pub why_not: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Entry {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: Source,

    #[serde(flatten)]
    pub data: KindData,

    pub natural_key_text: String,
    /// hex-encoded SHA-256 for JSON portability; stored as BLOB in SQLite
    pub natural_key_hash: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub superseded_by: Option<Ulid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteInput {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: Source,

    #[serde(flatten)]
    pub data: KindData,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub supersedes: Option<Ulid>,

    #[serde(default = "default_dedup")]
    pub dedup: DedupMode,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proceed_token: Option<String>,
}

const fn default_dedup() -> DedupMode {
    DedupMode::Check
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DedupMode {
    Check,
    Force,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WriteResult {
    Written {
        #[schemars(with = "String")]
        id: Ulid,
        version: u32,
    },
    DuplicatesFound {
        candidates: Vec<DuplicateCandidate>,
        proceed_token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DuplicateCandidate {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub kind: EntryKind,
    pub title: String,
    pub natural_key_text: String,
    /// Which layer caught this candidate.
    pub layer: DedupLayer,
    /// For Layer 1 this is 1.0; for Layer 2 this is the bm25 score (negative, smaller = better).
    pub score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DedupLayer {
    Exact,
    Fts,
}
