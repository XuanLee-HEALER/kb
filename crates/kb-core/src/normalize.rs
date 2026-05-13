use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::entry::{EntryKind, KindData};

/// ASCII Unit Separator. Used to join multi-field natural keys without colliding with content.
pub const FIELD_SEP: char = '\u{1F}';

/// Apply the normalize rules in order:
/// 1. NFKC
/// 2. lowercase fold
/// 3. whitespace collapse (any run of `\t \r\n \u{3000}` etc → single ASCII space, trim ends)
///
/// Punctuation is intentionally preserved — `tokio block_in_place` and `tokio block in place`
/// should hash differently.
#[must_use]
pub fn normalize_text(s: &str) -> String {
    let nfkc: String = s.nfkc().collect();
    let lower = nfkc.to_lowercase();

    let mut out = String::with_capacity(lower.len());
    let mut prev_ws = false;
    for c in lower.chars() {
        if c.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Result of natural-key derivation: the raw concatenation **before** the final normalize call,
/// the normalized text used for FTS / hash, and the SHA-256 hash (32 bytes).
#[derive(Debug, Clone)]
pub struct NaturalKey {
    pub text: String,
    pub hash: [u8; 32],
}

impl NaturalKey {
    #[must_use]
    pub fn hash_hex(&self) -> String {
        hex_encode(&self.hash)
    }
}

/// Derive the natural key text + hash for a given `KindData`.
///
/// Per spec (§5 各 type 的 NK 派生):
/// - Fact            : `normalize(claim)`
/// - ProblemSolution : `normalize(sort(symptoms).join(",") + \x1F + root_cause)`
/// - Lesson          : `normalize(trap)`
/// - Decision        : `normalize(context + \x1F + decision)`
/// - Heuristic       : `normalize(problem_class + \x1F + pattern)`
#[must_use]
pub fn natural_key(data: &KindData) -> NaturalKey {
    let raw = match data {
        KindData::Fact { claim, .. } => claim.clone(),
        KindData::ProblemSolution {
            symptoms,
            root_cause,
            ..
        } => {
            let mut s = symptoms.clone();
            s.sort();
            let joined = s.join(",");
            format!("{joined}{FIELD_SEP}{root_cause}")
        }
        KindData::Lesson { trap, .. } => trap.clone(),
        KindData::Decision {
            context, decision, ..
        } => format!("{context}{FIELD_SEP}{decision}"),
        KindData::Heuristic {
            problem_class,
            pattern,
            ..
        } => format!("{problem_class}{FIELD_SEP}{pattern}"),
    };

    let text = normalize_text(&raw);
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    NaturalKey { text, hash }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0F) as usize] as char);
    }
    out
}

/// Sanity check: this function is the single entry point all writers must call.
/// Tests live in `tests/` and `#[cfg(test)]` below.
#[must_use]
pub fn kind_of(data: &KindData) -> EntryKind {
    data.kind()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::nursery
)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::entry::{Alternative, Evidence, EvidenceKind};

    // ---- normalize_text rules ----

    #[test]
    fn nfkc_fullwidth_to_halfwidth() {
        // Fullwidth Latin → Halfwidth
        assert_eq!(normalize_text("Ｔｏｋｉｏ"), "tokio");
    }

    #[test]
    fn nfkc_combining_marks() {
        // 'é' as 'e' + combining acute → composed and lowercased
        assert_eq!(normalize_text("Cafe\u{301}"), "café");
    }

    #[test]
    fn lowercase_fold() {
        assert_eq!(normalize_text("Tokio"), "tokio");
        assert_eq!(normalize_text("RUST"), "rust");
    }

    #[test]
    fn whitespace_collapse_and_trim() {
        assert_eq!(normalize_text("  foo   bar\tbaz\n  "), "foo bar baz");
    }

    #[test]
    fn ideographic_space_treated_as_whitespace() {
        assert_eq!(normalize_text("foo\u{3000}bar"), "foo bar");
    }

    #[test]
    fn punctuation_preserved() {
        // The spec is explicit: don't strip punctuation. Underscore matters semantically.
        let a = normalize_text("tokio block_in_place");
        let b = normalize_text("tokio block in place");
        assert_ne!(a, b);
        assert_eq!(a, "tokio block_in_place");
    }

    // ---- natural_key per kind ----

    #[test]
    fn nk_fact_uses_claim_only() {
        let nk = natural_key(&KindData::Fact {
            claim: "  Kernel 5.15 + xt_TPROXY  绑定 TPROXY_ORIGINAL_DST_ADDR4  ".into(),
            evidence: vec![Evidence {
                kind: EvidenceKind::Output,
                content: "irrelevant".into(),
                captured_at: chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            }],
        });
        assert_eq!(
            nk.text,
            "kernel 5.15 + xt_tproxy 绑定 tproxy_original_dst_addr4"
        );
        assert_eq!(nk.hash.len(), 32);
    }

    #[test]
    fn nk_problem_solution_sorts_symptoms() {
        let a = natural_key(&KindData::ProblemSolution {
            problem: "p".into(),
            environment: "e".into(),
            symptoms: vec!["B".into(), "A".into()],
            root_cause: "RC".into(),
            solution: "s".into(),
            verification: None,
        });
        let b = natural_key(&KindData::ProblemSolution {
            problem: "p".into(),
            environment: "e".into(),
            symptoms: vec!["A".into(), "B".into()],
            root_cause: "RC".into(),
            solution: "s".into(),
            verification: None,
        });
        assert_eq!(a.text, b.text);
        assert_eq!(a.hash, b.hash);
        // Sanity: text contains the unit separator before root_cause
        assert!(a.text.contains('\u{1F}'));
        assert!(a.text.ends_with("rc"));
    }

    #[test]
    fn nk_lesson_uses_trap_only() {
        let nk = natural_key(&KindData::Lesson {
            trap: "RAII drop order assumed top-down".into(),
            correction: "actually bottom-up".into(),
            why: "spec wording".into(),
            context: None,
        });
        assert_eq!(nk.text, "raii drop order assumed top-down");
    }

    #[test]
    fn nk_decision_joins_context_and_decision() {
        let nk = natural_key(&KindData::Decision {
            context: "Need async runtime for KB write path".into(),
            decision: "Pick tokio".into(),
            rationale: "ecosystem".into(),
            alternatives: vec![Alternative {
                option: "smol".into(),
                why_not: "smaller ecosystem".into(),
            }],
            tradeoffs: vec!["bigger binary".into()],
        });
        assert!(nk.text.contains('\u{1F}'));
        let parts: Vec<&str> = nk.text.split('\u{1F}').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "need async runtime for kb write path");
        assert_eq!(parts[1], "pick tokio");
    }

    #[test]
    fn nk_heuristic_joins_class_and_pattern() {
        let nk = natural_key(&KindData::Heuristic {
            problem_class: "Network connectivity diagnosis".into(),
            pattern: "Layer up: cable → link → ip → route → fw → app".into(),
            limits: Some("Doesn't cover BGP".into()),
        });
        let parts: Vec<&str> = nk.text.split('\u{1F}').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("network"));
        assert!(parts[1].starts_with("layer up"));
    }

    // ---- hash properties ----

    #[test]
    fn equal_text_equal_hash() {
        let a = natural_key(&KindData::Fact {
            claim: "Foo".into(),
            evidence: vec![],
        });
        let b = natural_key(&KindData::Fact {
            claim: "FOO".into(),
            evidence: vec![],
        });
        assert_eq!(a.hash, b.hash);
    }

    #[test]
    fn different_text_different_hash() {
        let a = natural_key(&KindData::Fact {
            claim: "foo".into(),
            evidence: vec![],
        });
        let b = natural_key(&KindData::Fact {
            claim: "bar".into(),
            evidence: vec![],
        });
        assert_ne!(a.hash, b.hash);
    }
}
