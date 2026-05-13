// Server-side Markdown rendering for entry bodies.
//
// Post-processes the marked output: each [[ULID]] reference becomes a chip
// with the linked entry's title injected (resolved server-side from the
// kb-server). Falls back to the bare ULID when the lookup fails.

import { marked } from "marked";
import { kb } from "./kb-client";

const ULID_RE = /\[\[([0-9A-HJKMNP-TV-Z]{26})\]\]/g;

export async function render(body: string): Promise<string> {
  const html = await marked.parse(body, { gfm: true, breaks: true });

  // Collect ULIDs referenced.
  const ulids = new Set<string>();
  body.replace(ULID_RE, (_m, u: string) => {
    ulids.add(u);
    return _m;
  });
  if (ulids.size === 0) return html;

  // Resolve titles concurrently.
  const titles: Record<string, string> = {};
  await Promise.all(
    [...ulids].map(async (u) => {
      try {
        const e = await kb.get(u);
        titles[u] = e.title || u;
      } catch {
        titles[u] = u;
      }
    }),
  );

  return html.replace(ULID_RE, (_m, u: string) => {
    const title = titles[u] ?? u;
    const trimmed = title.length > 40 ? title.slice(0, 40) + "…" : title;
    return `<a class="ulid-chip" href="/entry/${u}" title="${escapeAttr(title)}"><span class="arrow">↗</span><span class="title">${escapeHtml(trimmed)}</span></a>`;
  });
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
function escapeAttr(s: string): string {
  return escapeHtml(s);
}

export function summary(body: string, n = 120): string {
  const first = body.replace(/\s+/g, " ").trim();
  if (first.length <= n) return first;
  return `${first.slice(0, n)}…`;
}
