import type { EntryKind } from "../lib/types";

interface Props {
  kind: EntryKind;
  prefill?: Record<string, unknown>;
  /** When true (default), the field group renders with `hidden`; the kind-switcher
   *  client script un-hides whichever matches the selected kind. On /edit we
   *  render only the matching group and force it visible. */
  hiddenByDefault?: boolean;
}

function s(prefill: Record<string, unknown> | undefined, k: string): string {
  if (!prefill) return "";
  const v = prefill[k];
  return typeof v === "string" ? v : "";
}
function lines(prefill: Record<string, unknown> | undefined, k: string): string {
  if (!prefill) return "";
  const v = prefill[k];
  if (Array.isArray(v)) return (v as string[]).join("\n");
  return "";
}
function altsLines(prefill: Record<string, unknown> | undefined): string {
  if (!prefill) return "";
  const v = prefill.alternatives;
  if (!Array.isArray(v)) return "";
  return (v as Array<{ option: string; why_not: string }>)
    .map((a) => `${a.option} — ${a.why_not}`)
    .join("\n");
}
function evidenceLines(prefill: Record<string, unknown> | undefined): string {
  if (!prefill) return "";
  const v = prefill.evidence;
  if (!Array.isArray(v)) return "";
  return (v as Array<{ kind: string; content: string }>)
    .map((e) => `${e.kind}::${e.content}`)
    .join("\n");
}

export function KindFieldsForm({ kind, prefill, hiddenByDefault = true }: Props) {
  return (
    <div data-kind-fields={kind} hidden={hiddenByDefault}>
      {kind === "Fact" && (
        <>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>claim</span>
              <span class="hint">一句已验证的陈述</span>
            </div>
            <textarea
              class="fld-textarea"
              name="claim"
              rows={2}
              placeholder="One-sentence verified statement…"
            >
              {s(prefill, "claim")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>evidence</span>
              <span class="hint">
                至少一条 · KIND::content per line · Url / Output / SelfVerification
              </span>
            </div>
            <textarea
              class="fld-textarea mono"
              name="evidence"
              rows={3}
              placeholder={"Url::https://…\nOutput::$ cmd → …"}
            >
              {evidenceLines(prefill)}
            </textarea>
          </div>
        </>
      )}

      {kind === "ProblemSolution" && (
        <>
          <div class="fld">
            <div class="fld-label">
              <span>problem</span>
            </div>
            <textarea
              class="fld-textarea"
              name="problem"
              rows={2}
              placeholder="One-sentence description of what went wrong."
            >
              {s(prefill, "problem")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>environment</span>
              <span class="hint">版本号、平台</span>
            </div>
            <input
              class="fld-input"
              name="environment"
              value={s(prefill, "environment")}
              placeholder="tokio 1.40, macOS 14.6, …"
              style="font-family: var(--font-mono); font-size: 13px"
            />
          </div>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>symptoms</span>
              <span class="hint">一行一条 · one per line</span>
            </div>
            <textarea
              class="fld-textarea"
              name="symptoms"
              rows={3}
              placeholder={"p99 spikes 8 → 4200ms\nstopping containers does not reclaim memory"}
            >
              {lines(prefill, "symptoms")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>root_cause</span>
            </div>
            <textarea
              class="fld-textarea"
              name="root_cause"
              rows={2}
              placeholder="What actually caused it. Not the symptom — the cause."
            >
              {s(prefill, "root_cause")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>solution</span>
            </div>
            <textarea
              class="fld-textarea"
              name="solution"
              rows={2}
              placeholder="The fix that worked."
            >
              {s(prefill, "solution")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>verification</span>
              <span class="hint">optional · 怎么确认修好了</span>
            </div>
            <textarea
              class="fld-textarea"
              name="verification"
              rows={2}
              placeholder="How you confirmed the fix worked."
            >
              {s(prefill, "verification")}
            </textarea>
          </div>
        </>
      )}

      {kind === "Lesson" && (
        <>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>trap</span>
              <span class="hint">"It looks like X..."</span>
            </div>
            <textarea
              class="fld-textarea"
              name="trap"
              rows={2}
              placeholder="It looks like X (the misleading belief)."
            >
              {s(prefill, "trap")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>correction</span>
            </div>
            <textarea
              class="fld-textarea"
              name="correction"
              rows={2}
              placeholder="What is actually true."
            >
              {s(prefill, "correction")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>why</span>
              <span class="hint">为什么这个陷阱会误导人</span>
            </div>
            <textarea
              class="fld-textarea"
              name="why"
              rows={2}
              placeholder="What makes the trap convincing."
            >
              {s(prefill, "why")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>context</span>
              <span class="hint">optional</span>
            </div>
            <textarea
              class="fld-textarea"
              name="lesson_context"
              rows={2}
              placeholder="When this trap arises."
            >
              {s(prefill, "context")}
            </textarea>
          </div>
        </>
      )}

      {kind === "Decision" && (
        <>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>context</span>
            </div>
            <textarea
              class="fld-textarea"
              name="dec_context"
              rows={2}
              placeholder="The situation in which you needed to decide."
            >
              {s(prefill, "context")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>decision</span>
            </div>
            <textarea class="fld-textarea" name="decision" rows={2} placeholder="What you chose.">
              {s(prefill, "decision")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>rationale</span>
            </div>
            <textarea
              class="fld-textarea"
              name="rationale"
              rows={3}
              placeholder="Why this, not the others."
            >
              {s(prefill, "rationale")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>alternatives</span>
              <span class="hint">一行一条 · "option — why_not"</span>
            </div>
            <textarea
              class="fld-textarea mono"
              name="alternatives"
              rows={3}
              placeholder={
                "Next.js — overkill for a single-user form-driven UI\nhtmx — TS-everywhere preference is friction"
              }
            >
              {altsLines(prefill)}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>tradeoffs</span>
            </div>
            <textarea
              class="fld-textarea"
              name="tradeoffs"
              rows={2}
              placeholder="What you give up by choosing this. One per line."
            >
              {lines(prefill, "tradeoffs")}
            </textarea>
          </div>
        </>
      )}

      {kind === "Heuristic" && (
        <>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>problem_class</span>
            </div>
            <textarea
              class="fld-textarea"
              name="problem_class"
              rows={2}
              placeholder="When does this heuristic apply?"
            >
              {s(prefill, "problem_class")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span class="nk">◆</span>
              <span>pattern</span>
              <span class="hint">思考模式(不是步骤)</span>
            </div>
            <textarea
              class="fld-textarea"
              name="pattern"
              rows={3}
              placeholder="How to think about this class. Not commands."
            >
              {s(prefill, "pattern")}
            </textarea>
          </div>
          <div class="fld">
            <div class="fld-label">
              <span>limits</span>
              <span class="hint">optional · 什么时候不适用</span>
            </div>
            <textarea
              class="fld-textarea"
              name="limits"
              rows={2}
              placeholder="When this heuristic breaks down."
            >
              {s(prefill, "limits")}
            </textarea>
          </div>
        </>
      )}
    </div>
  );
}
