// Per-kind metadata mirroring the design's KIND_META.
// Used by the picker / segmented control / sidebar / badges.

import type { EntryKind } from "./types";

export interface KindMeta {
  slug: string;
  cls: string; // CSS class suffix: 'fact' | 'ps' | 'lesson' | 'decision' | 'heuristic'
  zh: string;
  blurb: string;
  blurbZh: string;
  nkList: string[];
}

export const KINDS: readonly EntryKind[] = [
  "Fact",
  "ProblemSolution",
  "Lesson",
  "Decision",
  "Heuristic",
] as const;

export const KIND_META: Record<EntryKind, KindMeta> = {
  Fact: {
    slug: "fact",
    cls: "fact",
    zh: "事实",
    blurb: "A verified, atomic claim — needs evidence.",
    blurbZh: "一句已验证的事实陈述,必须附带证据。",
    nkList: ["claim"],
  },
  ProblemSolution: {
    slug: "ps",
    cls: "ps",
    zh: "问题方案",
    blurb: "Symptoms + root cause + the fix that worked.",
    blurbZh: "症状、根因、起作用的解法——下次复现可直接套用。",
    nkList: ["symptoms", "root_cause"],
  },
  Lesson: {
    slug: "lesson",
    cls: "lesson",
    zh: "教训",
    blurb: 'A trap — "looks like X, but actually Y."',
    blurbZh: '"看起来是 X 但其实是 Y" 的认知陷阱。',
    nkList: ["trap"],
  },
  Decision: {
    slug: "decision",
    cls: "decision",
    zh: "决策",
    blurb: "Your deliberate choice + the alternatives you rejected.",
    blurbZh: "你的取舍:选了什么,放弃了什么,为什么。",
    nkList: ["context", "decision"],
  },
  Heuristic: {
    slug: "heuristic",
    cls: "heuristic",
    zh: "启发法",
    blurb: "A pattern of thought — when this class shows up, think like this.",
    blurbZh: "一种思考模式(不是步骤),遇到这类问题时怎么想。",
    nkList: ["problem_class", "pattern"],
  },
};

export function kindCls(kind: EntryKind): string {
  return KIND_META[kind].cls;
}

export function fmtDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const days = Math.floor(diff / 86400000);
  if (days < 1) return "today";
  if (days < 2) return "yesterday";
  if (days < 30) return `${days}d ago`;
  if (days < 365) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

export function fmtDateLong(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}
