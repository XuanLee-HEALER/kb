import type { Entry } from "../lib/types";

interface Props {
  entry: Entry;
}

function str(e: Entry, k: string): string {
  const v = (e as unknown as Record<string, unknown>)[k];
  return typeof v === "string" ? v : "";
}
function arr(e: Entry, k: string): string[] {
  const v = (e as unknown as Record<string, unknown>)[k];
  return Array.isArray(v) ? (v as string[]) : [];
}
function evidenceList(e: Entry): Array<{ kind: string; content: string; captured_at: string }> {
  const v = (e as unknown as Record<string, unknown>).evidence;
  return Array.isArray(v)
    ? (v as Array<{ kind: string; content: string; captured_at: string }>)
    : [];
}
function altsList(e: Entry): Array<{ option: string; why_not: string }> {
  const v = (e as unknown as Record<string, unknown>).alternatives;
  return Array.isArray(v) ? (v as Array<{ option: string; why_not: string }>) : [];
}

export function KindFieldsDetail({ entry: e }: Props) {
  if (e.kind === "Fact") {
    return (
      <>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>claim</span>
          </div>
          <div class="field-value">{str(e, "claim")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>evidence</span>
          </div>
          <div style="display: flex; flex-direction: column; gap: 8px">
            {evidenceList(e).map((ev) => (
              <div class="field-evidence">
                <span class="kind">{ev.kind}</span>
                <span class="content">{ev.content}</span>
                <span class="date">captured · {ev.captured_at}</span>
              </div>
            ))}
          </div>
        </div>
      </>
    );
  }

  if (e.kind === "ProblemSolution") {
    return (
      <>
        <div class="field">
          <div class="field-label">
            <span>problem</span>
          </div>
          <div class="field-value">{str(e, "problem")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>environment</span>
          </div>
          <div class="field-value mono">{str(e, "environment")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>symptoms</span>
          </div>
          <ul class="field-list">
            {arr(e, "symptoms").map((s) => (
              <li>{s}</li>
            ))}
          </ul>
        </div>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>root_cause</span>
          </div>
          <div class="field-value">{str(e, "root_cause")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>solution</span>
          </div>
          <div class="field-value">{str(e, "solution")}</div>
        </div>
        {str(e, "verification") && (
          <div class="field">
            <div class="field-label">
              <span>verification</span>
            </div>
            <div class="field-value">{str(e, "verification")}</div>
          </div>
        )}
      </>
    );
  }

  if (e.kind === "Lesson") {
    return (
      <>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>trap</span>
          </div>
          <div class="field-value">{str(e, "trap")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>correction</span>
          </div>
          <div class="field-value">{str(e, "correction")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>why</span>
          </div>
          <div class="field-value">{str(e, "why")}</div>
        </div>
        {str(e, "context") && (
          <div class="field">
            <div class="field-label">
              <span>context</span>
            </div>
            <div class="field-value">{str(e, "context")}</div>
          </div>
        )}
      </>
    );
  }

  if (e.kind === "Decision") {
    return (
      <>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>context</span>
          </div>
          <div class="field-value">{str(e, "context")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>decision</span>
          </div>
          <div class="field-value">{str(e, "decision")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span>rationale</span>
          </div>
          <div class="field-value">{str(e, "rationale")}</div>
        </div>
        {altsList(e).length > 0 && (
          <div class="field">
            <div class="field-label">
              <span>alternatives</span>
            </div>
            <div>
              {altsList(e).map((a) => (
                <div class="alt">
                  <span class="opt">{a.option}</span>
                  <span class="why">
                    <span class="why-label">why_not:&nbsp;</span>
                    {a.why_not}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
        {arr(e, "tradeoffs").length > 0 && (
          <div class="field">
            <div class="field-label">
              <span>tradeoffs</span>
            </div>
            <div>
              {arr(e, "tradeoffs").map((t) => (
                <div class="tradeoff">{t}</div>
              ))}
            </div>
          </div>
        )}
      </>
    );
  }

  if (e.kind === "Heuristic") {
    return (
      <>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>problem_class</span>
          </div>
          <div class="field-value">{str(e, "problem_class")}</div>
        </div>
        <div class="field">
          <div class="field-label">
            <span class="nk">◆ NK</span>
            <span>pattern</span>
          </div>
          <div class="field-value">{str(e, "pattern")}</div>
        </div>
        {str(e, "limits") && (
          <div class="field">
            <div class="field-label">
              <span>limits</span>
            </div>
            <div class="field-value">{str(e, "limits")}</div>
          </div>
        )}
      </>
    );
  }

  return null;
}
