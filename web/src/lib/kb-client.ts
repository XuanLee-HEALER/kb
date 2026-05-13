// Server-side fetch wrapper. Imported only from .astro frontmatter or
// `src/pages/api/*` endpoints — never from client bundles, so KB_TOKEN never
// leaks to the browser.

import type { Entry, SearchHit, SearchQuery, Stats, WriteResult } from "./types";

const URL_BASE = process.env.KB_URL ?? "http://127.0.0.1:7890";
const TOKEN = process.env.KB_TOKEN ?? "";

function headers(): Record<string, string> {
  const h: Record<string, string> = { "content-type": "application/json" };
  if (TOKEN) h.authorization = `Bearer ${TOKEN}`;
  return h;
}

async function req<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${URL_BASE}${path}`, {
    method,
    headers: headers(),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`kb ${method} ${path} failed (${res.status}): ${text}`);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const kb = {
  search(q: SearchQuery): Promise<SearchHit[]> {
    return req<SearchHit[]>("POST", "/api/search", q);
  },
  recent(n = 20): Promise<SearchHit[]> {
    return req<SearchHit[]>("GET", `/api/recent?n=${n}`);
  },
  get(id: string): Promise<Entry> {
    return req<Entry>("GET", `/api/entries/${id}`);
  },
  write(input: unknown): Promise<WriteResult> {
    return req<WriteResult>("POST", "/api/entries", input);
  },
  update(id: string, partial: unknown): Promise<{ id: string; version: number }> {
    return req("PATCH", `/api/entries/${id}`, partial);
  },
  deprecate(id: string, reason: string): Promise<void> {
    return req("POST", `/api/entries/${id}/deprecate`, { reason });
  },
  stats(): Promise<Stats> {
    return req<Stats>("GET", "/api/stats");
  },
};
