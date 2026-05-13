// Render entry body as HTML. Post-process to turn `[[ULID]]` into anchor tags.

import { marked } from "marked";

const ULID_RE = /\[\[([0-9A-HJKMNP-TV-Z]{26})\]\]/g;

export async function render(body: string): Promise<string> {
  const html = await marked.parse(body, { gfm: true, breaks: true });
  return html.replace(
    ULID_RE,
    (_m, ulid: string) => `<a class="entry-link" href="/entry/${ulid}">↗ ${ulid}</a>`,
  );
}

export function summary(body: string, n = 120): string {
  const first = body.replace(/\s+/g, " ").trim();
  if (first.length <= n) return first;
  return `${first.slice(0, n)}…`;
}
