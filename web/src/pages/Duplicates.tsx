// Sheet-style "duplicates found" response after a kb.write returned candidates.
// Rendered as a standalone HTML page (no Layout/sidebar — full focus on the
// resolution decision).

import { KIND_META } from "../lib/kind-meta";
import type { DuplicateCandidate, EntryKind } from "../lib/types";

interface Props {
  candidates: DuplicateCandidate[];
  proceedToken: string;
  inputKind: EntryKind;
  /** Hidden form-fields that recreate the original draft, so the user can
   *  re-submit via supersede or force-write without retyping. */
  hidden: Array<[string, string]>;
}

export function Duplicates({ candidates, proceedToken, inputKind, hidden }: Props) {
  const l1 = candidates.filter((c) => c.layer === "exact");
  const l2 = candidates.filter((c) => c.layer === "fts");

  function HiddenInputs() {
    return (
      <>
        {hidden.map(([k, v]) => (
          <input type="hidden" name={k} value={v} />
        ))}
      </>
    );
  }

  function CandidateRow({ c }: { c: DuplicateCandidate }) {
    const cls = KIND_META[c.kind]?.cls ?? "fact";
    const layer = c.layer === "exact" ? "l1" : "l2";
    const layerLabel = c.layer === "exact" ? "L1" : "L2";
    const layerKind = c.layer === "exact" ? "exact" : "fts";
    const nkLabel = KIND_META[c.kind]?.nkList.join(" + ") ?? "nk";
    return (
      <div class={`dup-row ${layer}`}>
        <div class="layer">
          <div class="pill">{layerLabel}</div>
          <span>{layerKind}</span>
        </div>
        <div class="info">
          <div class="topline">
            <a
              href={`/entry/${c.id}`}
              class={`badge icon-lucide kindfg-${cls}`}
              target="_blank"
              rel="noopener"
              style="text-decoration: none"
            >
              <span>{c.kind}</span>
            </a>
            <span class="title">{c.title || "(untitled)"}</span>
            {c.layer === "fts" && <span class="score">bm25 {c.score.toFixed(2)}</span>}
          </div>
          <div class="nk">
            <span class="nklbl">◆ {nkLabel}:</span>
            <span>{c.natural_key_text}</span>
          </div>
        </div>
        <div class="actions">
          <a
            class="btn"
            href={`/edit/${c.id}`}
            style="border-color: var(--sakya-accent-secondary); color: var(--sakya-accent-secondary)"
          >
            Update this
          </a>
          <form method="post" action="/api/write" style="margin: 0">
            <HiddenInputs />
            <input type="hidden" name="supersedes" value={c.id} />
            <button
              type="submit"
              class="btn"
              style="border-color: var(--sakya-accent-primary); color: var(--sakya-accent-primary)"
            >
              Supersede →
            </button>
          </form>
        </div>
      </div>
    );
  }

  return (
    <html lang="zh-CN">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>Duplicates · KB</title>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
        <link
          href="https://fonts.googleapis.com/css2?family=Geist:wght@300;400;500;600;700&family=Geist+Mono:wght@400;500;600&family=Noto+Serif+SC:wght@500;600&display=swap"
          rel="stylesheet"
        />
        <link rel="stylesheet" href="/styles/sakya-dorje.css" />
        <link rel="stylesheet" href="/styles/kb.css" />
      </head>
      <body>
        <div
          class="sheet-overlay"
          style="position: static; padding: 6vh 24px 12vh; background: transparent; backdrop-filter: none"
        >
          <div class="sheet" style="animation: none">
            <div class="sheet-head">
              <span class="stripe3" aria-hidden="true">
                <i />
                <i />
                <i />
              </span>
              <div class="text">
                <h2>Looks like you've written this before</h2>
                <div class="sub">
                  Found {candidates.length} candidate{candidates.length === 1 ? "" : "s"} that match
                  your new {inputKind} entry's natural key. Pick one to update or supersede — or
                  force-write as new if these are genuinely different.
                  <span class="zh">
                    在 NK 层面发现 {candidates.length}{" "}
                    条疑似重复。选择更新、取代,或在确认不重复时强制新建。
                  </span>
                </div>
              </div>
              <a class="x" href="/new" role="button">
                ✕
              </a>
            </div>

            <div class="sheet-body">
              {l1.length > 0 && (
                <>
                  <div class="sheet-section-h">
                    <span style="color: var(--sakya-accent-primary)">L1</span>
                    <span>· exact hash match · 极高把握</span>
                  </div>
                  {l1.map((c) => (
                    <CandidateRow c={c} />
                  ))}
                </>
              )}
              {l2.length > 0 && (
                <>
                  <div class="sheet-section-h">
                    <span style="color: var(--sakya-accent-secondary)">L2</span>
                    <span>· FTS bm25 similarity · 内容相近</span>
                  </div>
                  {l2.map((c) => (
                    <CandidateRow c={c} />
                  ))}
                </>
              )}
            </div>

            <div class="sheet-foot">
              <div class="info">
                <strong>None of these?</strong> If your new entry is genuinely different,
                force-write a new one. A{" "}
                <code style="font-family: var(--font-mono); color: var(--sakya-accent-primary)">
                  proceed_token
                </code>{" "}
                is attached and your draft is preserved.
                <br />
                <span style="color: var(--sakya-fg-disabled)">
                  都不是?确认不重复后可强制新建,草稿会保留。
                </span>
              </div>
              <a class="btn ghost" href="/new">
                Back to draft
              </a>
              <form method="post" action="/api/write" style="margin: 0">
                <HiddenInputs />
                <input type="hidden" name="dedup" value="force" />
                <input type="hidden" name="proceed_token" value={proceedToken} />
                <button type="submit" class="btn primary">
                  Force write as new →
                </button>
              </form>
            </div>
          </div>
        </div>
      </body>
    </html>
  );
}

// Serialize the original WriteInput into form-field key/value pairs for
// preservation across the duplicates interstitial.
export function serializeHidden(input: Record<string, unknown>): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  for (const [k, v] of Object.entries(input)) {
    if (v === undefined || v === null) continue;
    if (k === "kind") {
      out.push(["kind", String(v)]);
      continue;
    }
    if (k === "source" || k === "supersedes" || k === "proceed_token" || k === "dedup") continue;
    if (k === "tags") {
      out.push(["tags", (v as string[]).join(", ")]);
      continue;
    }
    if (k === "evidence") {
      const ev = v as Array<{ kind: string; content: string }>;
      out.push(["evidence", ev.map((e) => `${e.kind}::${e.content}`).join("\n")]);
      continue;
    }
    if (Array.isArray(v)) {
      out.push([formField(k), (v as string[]).join("\n")]);
      continue;
    }
    out.push([formField(k), String(v)]);
  }
  return out;
}

function formField(jsonKey: string): string {
  // Decision.context lives at form-name "dec_context" to avoid clashing with
  // Lesson.context (which uses "lesson_context"). Mirror that mapping here.
  if (jsonKey === "context") return "dec_context";
  return jsonKey;
}
