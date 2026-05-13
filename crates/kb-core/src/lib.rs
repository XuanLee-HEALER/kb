//! KB core types and normalize logic.
#![deny(unsafe_code)]

pub mod entry;
pub mod normalize;
pub mod source;

pub use entry::{
    Alternative, DedupLayer, DedupMode, DuplicateCandidate, Entry, EntryKind, Evidence,
    EvidenceKind, KindData, WriteInput, WriteResult,
};
pub use normalize::{natural_key, normalize_text, NaturalKey};
pub use source::Source;
