// kb-server-web · Hono on Bun, SSR-only.
//
// Routes:
//   GET  /                            list
//   GET  /entry/:id                   detail
//   GET  /new                         new entry form
//   GET  /edit/:id                    edit form
//   GET  /stats                       stats
//   POST /api/write                   create entry → 303 to /entry/:id, or duplicates sheet
//   POST /api/entries/:id/update      update entry → 303 back to /entry/:id
//   POST /api/entries/:id/deprecate   soft-delete  → 303 back to /entry/:id
//   GET  /api/search                  client-side palette proxy (no token to browser)
//
// Static:
//   /styles/*  →  public/styles/
//   /js/*      →  public/js/  (client bundle from src/scripts/app.ts)

import { Hono } from "hono";
import { serveStatic } from "hono/bun";
import { buildWriteInput } from "./lib/form";
import { kb } from "./lib/kb-client";
import { KINDS } from "./lib/kind-meta";
import { render } from "./lib/markdown";
import type { Entry, EntryKind, SearchHit, Stats } from "./lib/types";
import { Detail } from "./pages/Detail";
import { Duplicates, serializeHidden } from "./pages/Duplicates";
import { Edit } from "./pages/Edit";
import { List } from "./pages/List";
import { New } from "./pages/New";
import { Stats as StatsPage } from "./pages/Stats";

const app = new Hono();

// ── static ──────────────────────────────────────────────────────────────────
app.use("/styles/*", serveStatic({ root: "./public" }));
app.use("/js/*", serveStatic({ root: "./public" }));
app.use("/favicon.ico", serveStatic({ path: "./public/favicon.ico" }));
app.use("/favicon.svg", serveStatic({ path: "./public/favicon.svg" }));

// ── pages ───────────────────────────────────────────────────────────────────

app.get("/", async (c) => {
  const url = new URL(c.req.url);
  const q = url.searchParams.get("q") ?? "";
  const kindParam = url.searchParams.get("kind");
  const tag = url.searchParams.get("tag") ?? "";
  const includeDeprecated = url.searchParams.get("d") === "1";
  const activeKind = (
    KINDS.includes(kindParam as EntryKind) ? (kindParam as EntryKind) : undefined
  ) as EntryKind | undefined;

  let hits: SearchHit[] = [];
  let error: string | null = null;
  try {
    hits = await kb.search({
      kinds: activeKind ? [activeKind] : undefined,
      tag_prefixes: tag ? [tag] : undefined,
      query: q.trim() ? q : undefined,
      limit: 100,
      include_deprecated: includeDeprecated,
    });
  } catch (e) {
    error = (e as Error).message;
  }
  return c.html(
    <List
      hits={hits}
      error={error}
      q={q}
      activeKind={activeKind}
      tag={tag}
      includeDeprecated={includeDeprecated}
      url={url}
    />,
  );
});

app.get("/entry/:id", async (c) => {
  const id = c.req.param("id");
  let entry: Entry | null = null;
  let error: string | null = null;
  let supersedeeTitle: string | undefined;
  try {
    entry = await kb.get(id);
    if (entry?.superseded_by) {
      try {
        const s = await kb.get(entry.superseded_by);
        supersedeeTitle = s.title;
      } catch {
        /* ignore */
      }
    }
  } catch (e) {
    error = (e as Error).message;
  }
  const bodyHtml = entry?.body ? await render(entry.body) : "";
  return c.html(
    <Detail entry={entry} error={error} bodyHtml={bodyHtml} supersedeeTitle={supersedeeTitle} />,
  );
});

app.get("/new", async (c) => {
  const url = new URL(c.req.url);
  const k = url.searchParams.get("kind");
  const presetKind = (KINDS.includes(k as EntryKind) ? k : "ProblemSolution") as EntryKind;
  const supersedes = url.searchParams.get("supersedes") ?? "";

  let allTags: string[] = [];
  try {
    const hits = await kb.search({ limit: 300 });
    const s = new Set<string>();
    for (const h of hits) for (const t of h.tags) s.add(t);
    allTags = [...s].sort();
  } catch {
    /* offline OK */
  }
  return c.html(<New presetKind={presetKind} supersedes={supersedes} allTags={allTags} />);
});

app.get("/edit/:id", async (c) => {
  const id = c.req.param("id");
  let entry: Entry | null = null;
  let error: string | null = null;
  try {
    entry = await kb.get(id);
  } catch (e) {
    error = (e as Error).message;
  }
  let allTags: string[] = [];
  try {
    const hits = await kb.search({ limit: 300 });
    const s = new Set<string>();
    for (const h of hits) for (const t of h.tags) s.add(t);
    allTags = [...s].sort();
  } catch {
    /* offline OK */
  }
  return c.html(<Edit entry={entry} error={error} allTags={allTags} />);
});

app.get("/stats", async (c) => {
  let stats: Stats | null = null;
  let error: string | null = null;
  const activity: number[] = new Array(30).fill(0);
  try {
    stats = await kb.stats();
    const hits = await kb.search({ limit: 300, include_deprecated: true });
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    for (const h of hits) {
      const d = new Date(h.updated_at);
      d.setHours(0, 0, 0, 0);
      const days = Math.floor((today.getTime() - d.getTime()) / 86400000);
      if (days >= 0 && days < 30) {
        const i = 29 - days;
        activity[i] = (activity[i] ?? 0) + 1;
      }
    }
  } catch (e) {
    error = (e as Error).message;
  }
  return c.html(<StatsPage stats={stats} error={error} activity={activity} />);
});

// ── api ─────────────────────────────────────────────────────────────────────

app.get("/api/search", async (c) => {
  const url = new URL(c.req.url);
  const q = url.searchParams.get("q") ?? undefined;
  const limit = Number(url.searchParams.get("limit") ?? 200);
  const kindParam = url.searchParams.get("kind");
  const kinds = kindParam
    ? kindParam.split(",").filter((k): k is EntryKind => KINDS.includes(k as EntryKind))
    : undefined;
  const includeDeprecated = url.searchParams.get("d") === "1";
  try {
    const hits = await kb.search({
      query: q,
      limit,
      include_deprecated: includeDeprecated,
      kinds,
    });
    return c.json(hits);
  } catch (e) {
    return c.text((e as Error).message, 500);
  }
});

app.post("/api/write", async (c) => {
  let input: Record<string, unknown>;
  try {
    const form = await c.req.formData();
    input = buildWriteInput(form);
  } catch (e) {
    return c.text(`write failed: ${(e as Error).message}`, 400);
  }
  try {
    const res = await kb.write(input);
    if (res.status === "written") {
      return c.redirect(`/entry/${res.id}`, 303);
    }
    const hidden = serializeHidden(input);
    return c.html(
      <Duplicates
        candidates={res.candidates}
        proceedToken={res.proceed_token}
        inputKind={(input.kind as EntryKind) ?? "Fact"}
        hidden={hidden}
      />,
    );
  } catch (e) {
    return c.text(`write failed: ${(e as Error).message}`, 500);
  }
});

app.post("/api/entries/:id/update", async (c) => {
  const id = c.req.param("id");
  const form = await c.req.formData();
  const title = (form.get("title") as string | null)?.trim();
  const body = (form.get("body") as string | null) ?? "";
  const tagsRaw = (form.get("tags") as string | null) ?? "";
  const tags = tagsRaw
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);

  let kindData: ReturnType<typeof import("./lib/form").buildKindData>;
  try {
    kindData = (await import("./lib/form")).buildKindData(form);
  } catch (e) {
    return c.text((e as Error).message, 400);
  }
  const partial = { title, body, tags, ...kindData };
  try {
    await kb.update(id, partial);
  } catch (e) {
    return c.text(`update failed: ${(e as Error).message}`, 500);
  }
  return c.redirect(`/entry/${id}`, 303);
});

app.post("/api/entries/:id/deprecate", async (c) => {
  const id = c.req.param("id");
  const form = await c.req.formData();
  const reason = (form.get("reason") as string | null)?.trim();
  if (!reason) return c.text("reason required", 400);
  try {
    await kb.deprecate(id, reason);
  } catch (e) {
    return c.text(`deprecate failed: ${(e as Error).message}`, 500);
  }
  return c.redirect(`/entry/${id}`, 303);
});

// ── boot ────────────────────────────────────────────────────────────────────

const port = Number(process.env.PORT ?? 5101);

export default {
  port,
  fetch: app.fetch,
};
