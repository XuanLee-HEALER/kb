import type { Evidence, KindData } from "./types";

function trimOr<T extends string | undefined>(v: T): T {
  if (typeof v !== "string") return v;
  const t = v.trim();
  return (t === "" ? undefined : t) as T;
}

function lines(s: string | null): string[] {
  if (!s) return [];
  return s
    .split(/\r?\n/)
    .map((x) => x.trim())
    .filter(Boolean);
}

function parseEvidence(raw: string | null): Evidence[] {
  return lines(raw).map((line) => {
    const m = line.match(/^(Output|Url|SelfVerification)::(.*)$/);
    if (m?.[1] && m[2]) {
      return {
        kind: m[1] as Evidence["kind"],
        content: m[2],
        captured_at: new Date().toISOString(),
      };
    }
    return {
      kind: "SelfVerification" as const,
      content: line,
      captured_at: new Date().toISOString(),
    };
  });
}

export function buildKindData(form: FormData): KindData {
  const kind = form.get("kind") as string;
  const get = (k: string): string | null => {
    const v = form.get(k);
    return typeof v === "string" ? v : null;
  };
  const required = (k: string): string => {
    const v = (get(k) ?? "").trim();
    if (!v) throw new Error(`field '${k}' is required for kind ${kind}`);
    return v;
  };
  switch (kind) {
    case "Fact":
      return {
        kind: "Fact",
        claim: required("claim"),
        evidence: parseEvidence(get("evidence")),
      };
    case "ProblemSolution":
      return {
        kind: "ProblemSolution",
        problem: required("problem"),
        environment: required("environment"),
        symptoms: lines(get("symptoms")),
        root_cause: required("root_cause"),
        solution: required("solution"),
        verification: trimOr(get("verification") ?? undefined),
      };
    case "Lesson":
      return {
        kind: "Lesson",
        trap: required("trap"),
        correction: required("correction"),
        why: required("why"),
        context: trimOr(get("lesson_context") ?? undefined),
      };
    case "Decision":
      return {
        kind: "Decision",
        context: required("dec_context"),
        decision: required("decision"),
        rationale: required("rationale"),
        tradeoffs: lines(get("tradeoffs")),
      };
    case "Heuristic":
      return {
        kind: "Heuristic",
        problem_class: required("problem_class"),
        pattern: required("pattern"),
        limits: trimOr(get("limits") ?? undefined),
      };
    default:
      throw new Error(`unknown kind: ${kind}`);
  }
}

export function buildWriteInput(form: FormData): Record<string, unknown> {
  const title = (form.get("title") as string)?.trim() ?? "";
  if (!title) throw new Error("title is required");
  const body = (form.get("body") as string) ?? "";
  const tagsRaw = (form.get("tags") as string) ?? "";
  const tags = tagsRaw
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
  const supersedes = trimOr((form.get("supersedes") as string) ?? undefined);
  const dedup = (form.get("dedup") as string) ?? "check";
  const proceedToken = trimOr((form.get("proceed_token") as string) ?? undefined);

  const data = buildKindData(form);
  return {
    title,
    body,
    tags,
    source: { type: "Human" },
    supersedes,
    dedup,
    proceed_token: proceedToken,
    ...data,
  };
}
