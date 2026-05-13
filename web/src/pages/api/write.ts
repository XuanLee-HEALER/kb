import type { APIRoute } from "astro";
import { buildWriteInput } from "../../lib/form";
import { kb } from "../../lib/kb-client";
import { KIND_META } from "../../lib/kind-meta";
import type { DuplicateCandidate, EntryKind, WriteResult } from "../../lib/types";

export const POST: APIRoute = async ({ request, redirect }) => {
  let input: Record<string, unknown>;
  try {
    const form = await request.formData();
    input = buildWriteInput(form);
  } catch (e) {
    return errorPage((e as Error).message);
  }

  let res: WriteResult;
  try {
    res = await kb.write(input);
  } catch (e) {
    return errorPage((e as Error).message);
  }

  if (res.status === "written") {
    return redirect(`/entry/${res.id}`, 303);
  }
  return new Response(renderDuplicates(res.candidates, res.proceed_token, input), {
    status: 200,
    headers: { "content-type": "text/html; charset=utf-8" },
  });
};

function errorPage(msg: string): Response {
  return new Response(
    shell(`
    <div class="empty" style="margin: 12vh auto; max-width: 480px">
      <span class="stripe3-h" aria-hidden="true"><i></i><i></i><i></i></span>
      <h3>Write failed.</h3>
      <p>${escapeHtml(msg)}</p>
      <p style="margin-top: 18px"><a class="btn" href="/new">← Back to form</a></p>
    </div>`),
    { status: 400, headers: { "content-type": "text/html; charset=utf-8" } },
  );
}

function renderDuplicates(
  candidates: DuplicateCandidate[],
  proceedToken: string,
  input: Record<string, unknown>,
): string {
  const hidden = serializeHidden(input);
  const inputKind = (input.kind as EntryKind) ?? "Fact";

  const l1 = candidates.filter((c) => c.layer === "exact");
  const l2 = candidates.filter((c) => c.layer === "fts");

  const candidateRow = (c: DuplicateCandidate): string => {
    const cls = KIND_META[c.kind]?.cls ?? "fact";
    const layer = c.layer === "exact" ? "l1" : "l2";
    const layerLabel = c.layer === "exact" ? "L1" : "L2";
    const layerKind = c.layer === "exact" ? "exact" : "fts";
    const score = c.layer === "fts" ? `<span class="score">bm25 ${c.score.toFixed(2)}</span>` : "";
    const nkLabel = KIND_META[c.kind]?.nkList.join(" + ") ?? "nk";
    return `
<div class="dup-row ${layer}">
  <div class="layer">
    <div class="pill">${layerLabel}</div>
    <span>${layerKind}</span>
  </div>
  <div class="info">
    <div class="topline">
      <a href="/entry/${c.id}" class="badge icon-lucide kindfg-${cls}" target="_blank" style="text-decoration: none">
        <span>${escapeHtml(c.kind)}</span>
      </a>
      <span class="title">${escapeHtml(c.title || "(untitled)")}</span>
      ${score}
    </div>
    <div class="nk">
      <span class="nklbl">◆ ${escapeHtml(nkLabel)}:</span>
      <span>${escapeHtml(c.natural_key_text)}</span>
    </div>
  </div>
  <div class="actions">
    <a class="btn" href="/edit/${c.id}" style="border-color: var(--sakya-accent-secondary); color: var(--sakya-accent-secondary)">Update this</a>
    <form method="POST" action="/api/write" style="margin: 0">
      ${hidden}
      <input type="hidden" name="supersedes" value="${c.id}" />
      <button type="submit" class="btn" style="border-color: var(--sakya-accent-primary); color: var(--sakya-accent-primary)">Supersede →</button>
    </form>
  </div>
</div>`;
  };

  return shell(`
<div class="sheet-overlay" style="position: static; padding: 6vh 24px 12vh; background: transparent; backdrop-filter: none">
  <div class="sheet" style="animation: none">
    <div class="sheet-head">
      <span class="stripe3" aria-hidden="true"><i></i><i></i><i></i></span>
      <div class="text">
        <h2>Looks like you've written this before</h2>
        <div class="sub">
          Found ${candidates.length} candidate${candidates.length === 1 ? "" : "s"} that match your new ${escapeHtml(inputKind)} entry's natural key.
          Pick one to update or supersede — or force-write as new if these are genuinely different.
          <span class="zh">在 NK 层面发现 ${candidates.length} 条疑似重复。选择更新、取代,或在确认不重复时强制新建。</span>
        </div>
      </div>
      <a class="x" href="/new" role="button">✕</a>
    </div>

    <div class="sheet-body">
      ${
        l1.length > 0
          ? `
        <div class="sheet-section-h">
          <span style="color: var(--sakya-accent-primary)">L1</span>
          <span>· exact hash match · 极高把握</span>
        </div>
        ${l1.map(candidateRow).join("")}
      `
          : ""
      }
      ${
        l2.length > 0
          ? `
        <div class="sheet-section-h">
          <span style="color: var(--sakya-accent-secondary)">L2</span>
          <span>· FTS bm25 similarity · 内容相近</span>
        </div>
        ${l2.map(candidateRow).join("")}
      `
          : ""
      }
    </div>

    <div class="sheet-foot">
      <div class="info">
        <strong>None of these?</strong> If your new entry is genuinely different,
        force-write a new one. A <code style="font-family: var(--font-mono); color: var(--sakya-accent-primary)">proceed_token</code> is attached and your draft is preserved.
        <br /><span style="color: var(--sakya-fg-disabled)">都不是?确认不重复后可强制新建,草稿会保留。</span>
      </div>
      <a class="btn ghost" href="/new">Back to draft</a>
      <form method="POST" action="/api/write" style="margin: 0">
        ${hidden}
        <input type="hidden" name="dedup" value="force" />
        <input type="hidden" name="proceed_token" value="${escapeAttr(proceedToken)}" />
        <button type="submit" class="btn primary">Force write as new →</button>
      </form>
    </div>
  </div>
</div>
`);
}

function shell(inner: string): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Duplicates · KB</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Geist:wght@300;400;500;600;700&family=Geist+Mono:wght@400;500;600&family=Noto+Serif+SC:wght@500;600&display=swap" rel="stylesheet">
<link rel="stylesheet" href="/styles/sakya-dorje.css">
<link rel="stylesheet" href="/styles/kb.css">
</head>
<body>
${inner}
</body>
</html>`;
}

function serializeHidden(input: Record<string, unknown>): string {
  const result: Array<[string, string]> = [];
  for (const [k, v] of Object.entries(input)) {
    if (v === undefined || v === null) continue;
    if (k === "kind") {
      result.push(["kind", String(v)]);
      continue;
    }
    if (k === "source" || k === "supersedes" || k === "proceed_token" || k === "dedup") continue;
    if (k === "tags") {
      result.push(["tags", (v as string[]).join(", ")]);
      continue;
    }
    if (k === "evidence") {
      const ev = v as Array<{ kind: string; content: string }>;
      result.push(["evidence", ev.map((e) => `${e.kind}::${e.content}`).join("\n")]);
      continue;
    }
    if (Array.isArray(v)) {
      result.push([formField(k), (v as string[]).join("\n")]);
      continue;
    }
    result.push([formField(k), String(v)]);
  }
  return result
    .map(([k, v]) => `<input type="hidden" name="${escapeAttr(k)}" value="${escapeAttr(v)}">`)
    .join("\n");
}

function formField(jsonKey: string): string {
  if (jsonKey === "context") return "dec_context";
  return jsonKey;
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
