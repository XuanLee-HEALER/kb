import type { APIRoute } from "astro";
import { buildWriteInput } from "../../lib/form";
import { kb } from "../../lib/kb-client";
import type { DuplicateCandidate, WriteResult } from "../../lib/types";

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
  const body = `<!doctype html><meta charset="utf-8"><title>Error</title>
<link rel="stylesheet" href="/_astro/styles.css">
<div class="shell"><div class="callout"><strong>Write failed.</strong><br>${escapeHtml(msg)}</div>
<p><a class="button" href="/new">Back to form</a></p></div>`;
  return new Response(body, {
    status: 400,
    headers: { "content-type": "text/html; charset=utf-8" },
  });
}

function renderDuplicates(
  candidates: DuplicateCandidate[],
  proceedToken: string,
  input: Record<string, unknown>,
): string {
  const hidden = serializeHidden(input);
  const list = candidates
    .map((c) => {
      const layer = c.layer === "exact" ? "L1 exact" : "L2 fts";
      return `<li>
        <span class="chip kind-${c.kind}">${c.kind}</span>
        <a href="/entry/${c.id}">${escapeHtml(c.title)}</a>
        <span class="chip">${layer}</span>
        <div style="color: var(--text-muted); font-size: 12px; margin-top: 4px;">${escapeHtml(
          c.natural_key_text,
        )}</div>
      </li>`;
    })
    .join("");

  return `<!doctype html><meta charset="utf-8"><title>Duplicates</title>
<link rel="stylesheet" href="/styles.css">
<div class="shell">
  <h1 class="h1">Possible duplicates</h1>
  <p style="color: var(--text-secondary); margin-bottom: 18px;">
    Three options: <strong>update existing</strong>, <strong>supersede</strong> one, or
    <strong>force write</strong> if you're sure this is a different situation.
  </p>
  <ul class="entry-list">${list}</ul>

  <h2 class="h2">Force write as new</h2>
  <p style="color: var(--text-muted); font-size: 12.5px;">
    Body should reference the related entries via <code>[[ULID]]</code> for context.
  </p>
  <form action="/api/write" method="POST">
    ${hidden}
    <input type="hidden" name="dedup" value="force">
    <input type="hidden" name="proceed_token" value="${escapeHtml(proceedToken)}">
    <div class="actions">
      <button class="primary" type="submit">Force write</button>
      <a class="button" href="/new">Back</a>
    </div>
  </form>

  <h2 class="h2">Supersede one of the above</h2>
  <form action="/api/write" method="POST">
    ${hidden}
    <label class="field" for="supersedes">supersedes ULID</label>
    <input id="supersedes" name="supersedes" type="text" required>
    <div class="actions">
      <button class="primary" type="submit">Supersede + write</button>
    </div>
  </form>
</div>`;
}

function serializeHidden(input: Record<string, unknown>): string {
  // Persist the original form fields as hidden inputs so the user can re-submit
  // with dedup=force or supersedes set. Flatten KindData fields back to their
  // form names.
  const result: Array<[string, string]> = [];
  for (const [k, v] of Object.entries(input)) {
    if (v === undefined || v === null) continue;
    if (k === "kind") {
      result.push(["kind", String(v)]);
      continue;
    }
    if (k === "source" || k === "supersedes" || k === "proceed_token" || k === "dedup") {
      // skip — set by the page or by the user explicitly
      continue;
    }
    if (k === "tags") {
      result.push(["tags", (v as string[]).join(", ")]);
      continue;
    }
    if (k === "evidence") {
      const ev = v as { kind: string; content: string }[];
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
    .map(([k, v]) => `<input type="hidden" name="${escapeHtml(k)}" value="${escapeHtml(v)}">`)
    .join("\n");
}

function formField(jsonKey: string): string {
  // Map a few KindData JSON keys back to their HTML form names.
  if (jsonKey === "context") return "dec_context"; // Decision.context
  return jsonKey;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
