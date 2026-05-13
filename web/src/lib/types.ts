// Types mirror the Rust kb-core schema. Keep manually in sync until we add a
// codegen step.

export type EntryKind = "Fact" | "ProblemSolution" | "Lesson" | "Decision" | "Heuristic";

export const ALL_KINDS: readonly EntryKind[] = [
  "Fact",
  "ProblemSolution",
  "Lesson",
  "Decision",
  "Heuristic",
] as const;

export type EvidenceKind = "Output" | "Url" | "SelfVerification";

export interface Evidence {
  kind: EvidenceKind;
  content: string;
  captured_at: string;
}

export interface Alternative {
  option: string;
  why_not: string;
}

export type Source =
  | { type: "Human" }
  | { type: "ClaudeCode"; session_id: string; project: string; cwd: string }
  | { type: "Imported"; from: string; original_date: string };

export type KindData =
  | { kind: "Fact"; claim: string; evidence: Evidence[] }
  | {
      kind: "ProblemSolution";
      problem: string;
      environment: string;
      symptoms: string[];
      root_cause: string;
      solution: string;
      verification?: string;
    }
  | { kind: "Lesson"; trap: string; correction: string; why: string; context?: string }
  | {
      kind: "Decision";
      context: string;
      decision: string;
      rationale: string;
      alternatives?: Alternative[];
      tradeoffs?: string[];
    }
  | { kind: "Heuristic"; problem_class: string; pattern: string; limits?: string };

export interface Entry {
  id: string;
  title: string;
  body: string;
  tags: string[];
  source: Source;
  natural_key_text: string;
  natural_key_hash: string;
  created_at: string;
  updated_at: string;
  version: number;
  deprecated_at?: string;
  deprecation_reason?: string;
  superseded_by?: string;
  // KindData flattened
  kind: EntryKind;
  // remaining type_data fields are interleaved at the top level by serde flatten
  // (we treat the result as a discriminated union below)
  [k: string]: unknown;
}

export interface SearchHit {
  id: string;
  kind: EntryKind;
  title: string;
  summary_line: string;
  tags: string[];
  created_at: string;
  updated_at: string;
  deprecated_at?: string | null;
  score?: number | null;
}

export interface Stats {
  total: number;
  active: number;
  deprecated: number;
  by_kind: Array<{ kind: EntryKind; active: number; deprecated: number }>;
}

export type DedupLayer = "exact" | "fts";

export interface DuplicateCandidate {
  id: string;
  kind: EntryKind;
  title: string;
  natural_key_text: string;
  layer: DedupLayer;
  score: number;
}

export type WriteResult =
  | { status: "written"; id: string; version: number }
  | { status: "duplicates_found"; candidates: DuplicateCandidate[]; proceed_token: string };

export interface SearchQuery {
  kinds?: EntryKind[];
  tag_prefixes?: string[];
  query?: string;
  limit?: number;
  include_deprecated?: boolean;
}
