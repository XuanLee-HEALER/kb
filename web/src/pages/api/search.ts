// Same-origin proxy for client-side Cmd-K palette. The Astro server holds
// KB_TOKEN; the browser fetches this endpoint without auth.

import type { APIRoute } from "astro";
import { kb } from "../../lib/kb-client";
import { KINDS } from "../../lib/kind-meta";
import type { EntryKind } from "../../lib/types";

export const GET: APIRoute = async ({ url }) => {
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
    return new Response(JSON.stringify(hits), {
      headers: { "content-type": "application/json" },
    });
  } catch (e) {
    return new Response((e as Error).message, { status: 500 });
  }
};
