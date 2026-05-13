import { KindBadge } from "../components/KindBadge";
import { Layout } from "../components/Layout";
import { Stripe3 } from "../components/Stripe3";
import { TagMini } from "../components/TagMini";
import { fmtDate, KIND_META, KINDS } from "../lib/kind-meta";
import type { EntryKind, SearchHit } from "../lib/types";

interface Props {
  hits: SearchHit[];
  error: string | null;
  q: string;
  activeKind?: EntryKind;
  tag: string;
  includeDeprecated: boolean;
  url: URL;
}

export function List(props: Props) {
  const { hits, error, q, activeKind, tag, includeDeprecated, url } = props;

  const baseHits = includeDeprecated ? hits : hits.filter((h) => !h.deprecated_at);
  const kindCounts: Record<string, number> = {};
  for (const k of KINDS) kindCounts[k] = baseHits.filter((h) => h.kind === k).length;

  function preserveQuery(setKind?: EntryKind | null, toggleDep?: boolean): string {
    const u = new URL(url);
    if (setKind === null) u.searchParams.delete("kind");
    else if (setKind) u.searchParams.set("kind", setKind);
    if (toggleDep) {
      if (includeDeprecated) u.searchParams.delete("d");
      else u.searchParams.set("d", "1");
    }
    return `${u.pathname}?${u.searchParams.toString()}`;
  }

  function clearParam(name: string): string {
    const u = new URL(url);
    u.searchParams.delete(name);
    return `${u.pathname}?${u.searchParams.toString()}`;
  }

  const pageTitle = activeKind
    ? activeKind
    : tag
      ? `tag · ${tag}`
      : includeDeprecated && !q
        ? "Deprecated"
        : "All entries";

  return (
    <Layout
      title={pageTitle}
      active="list"
      activeKind={activeKind}
      activeTag={tag || null}
      showingDeprecated={includeDeprecated && !activeKind && !tag}
    >
      <div data-page="list">
        <div class="ph">
          <div class="ph-title">
            <div class="ph-eyebrow">
              <Stripe3 vertical={false} />
              <span>library · 库</span>
            </div>
            <h1>
              {activeKind ? (
                activeKind
              ) : tag ? (
                <>
                  tagged{" "}
                  <span style="font-family: var(--font-mono); color: var(--sakya-accent-primary)">
                    {tag}
                  </span>
                </>
              ) : (
                "All entries"
              )}
            </h1>
            <div class="ph-sub">
              {hits.length} {hits.length === 1 ? "entry" : "entries"}
              {q && <> · searching "{q}"</>}
            </div>
          </div>
        </div>

        <form class="lst-toolbar" method="get" action="/" role="search">
          {activeKind && <input type="hidden" name="kind" value={activeKind} />}
          {tag && <input type="hidden" name="tag" value={tag} />}
          {includeDeprecated && <input type="hidden" name="d" value="1" />}
          <div class="lst-search">
            <svg
              width="15"
              height="15"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="11" cy="11" r="7" />
              <path d="m21 21-4.3-4.3" />
            </svg>
            <input
              type="search"
              name="q"
              value={q}
              autocomplete="off"
              placeholder="Search title, body, tags…   |   按标题、正文、标签搜索"
            />
            {q ? (
              <a class="hint" href={clearParam("q")}>
                clear ✕
              </a>
            ) : (
              <span class="hint">FTS · libsimple · bm25</span>
            )}
          </div>

          <div class="lst-filters">
            {KINDS.map((k) => {
              const on = activeKind === k;
              const cls = KIND_META[k].cls;
              return (
                <a
                  class={`chip ${on ? "on" : ""} kindfg-${cls}`}
                  href={preserveQuery(on ? null : k)}
                >
                  <span class={`dot kinddot-${cls}`} />
                  <span>{k}</span>
                  <span class="count">{kindCounts[k]}</span>
                </a>
              );
            })}
            <a
              class="chip"
              href={preserveQuery(undefined, true)}
              style={`color: ${includeDeprecated ? "var(--sakya-fg-secondary)" : "var(--sakya-fg-tertiary)"}; border-style: ${includeDeprecated ? "solid" : "dashed"}`}
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                {includeDeprecated ? (
                  <path d="M5 12l5 5 9-11" />
                ) : (
                  <path d="M5 5l14 14M19 5L5 19" />
                )}
              </svg>
              <span>show deprecated</span>
            </a>
            {tag && (
              <a
                class="chip on tag"
                style="color: var(--sakya-accent-primary)"
                href={clearParam("tag")}
              >
                <span>tag:{tag}</span>
                <span class="x">✕</span>
              </a>
            )}
          </div>
        </form>

        {error && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>kb-server unreachable</h3>
            <p style="max-width: 380px; margin-top: 8px;">{error}</p>
          </div>
        )}

        {!error && hits.length === 0 && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>No entries match.</h3>
            <p style="max-width: 380px; margin-top: 8px;">
              Try clearing a filter, or <a href="/">show everything</a>.
              <br />
              <span style="color: var(--sakya-fg-disabled)">清空筛选条件,或创建一条新条目。</span>
            </p>
          </div>
        )}

        {!error && hits.length > 0 && (
          <div class="lst comfortable">
            {hits.map((h) => (
              <a class={`row ${h.deprecated_at ? "deprecated" : ""}`} href={`/entry/${h.id}`}>
                <div class="title-line">
                  <KindBadge kind={h.kind} />
                  <span class="title">{h.title || "(untitled)"}</span>
                </div>
                <div class="meta">
                  <span>{fmtDate(h.updated_at)}</span>
                  {q && h.score != null ? (
                    <span class="score">bm25 {h.score.toFixed(1)}</span>
                  ) : (
                    <span style="color: var(--sakya-fg-disabled); font-size: 10px">
                      id {h.id.slice(0, 8)}
                    </span>
                  )}
                </div>
                {h.summary_line && <div class="summary">{h.summary_line}</div>}
                {h.tags.length > 0 && (
                  <div class="tags">
                    {h.tags.map((t) => (
                      <TagMini tag={t} />
                    ))}
                  </div>
                )}
              </a>
            ))}
          </div>
        )}
      </div>
    </Layout>
  );
}
