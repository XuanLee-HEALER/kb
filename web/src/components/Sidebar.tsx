import { kb } from "../lib/kb-client";
import { KIND_META, KINDS } from "../lib/kind-meta";
import type { EntryKind, Stats } from "../lib/types";
import { Stripe3 } from "./Stripe3";

interface Props {
  active?: "list" | "stats" | "new";
  activeKind?: EntryKind;
  activeTag?: string | null;
  showingDeprecated?: boolean;
}

type TagCount = { count: number; children: Record<string, number> };

export async function Sidebar(props: Props) {
  const { active, activeKind, activeTag, showingDeprecated } = props;

  let stats: Stats | null = null;
  const tagCounts: Record<string, TagCount> = {};
  let totalEntries = 0;

  try {
    stats = await kb.stats();
    const hits = await kb.search({ limit: 200, include_deprecated: false });
    for (const h of hits) {
      for (const t of h.tags) {
        const slash = t.indexOf("/");
        if (slash > 0) {
          const top = t.slice(0, slash);
          const rest = t.slice(slash + 1);
          if (!tagCounts[top]) tagCounts[top] = { count: 0, children: {} };
          tagCounts[top].count += 1;
          tagCounts[top].children[rest] = (tagCounts[top].children[rest] ?? 0) + 1;
        } else {
          if (!tagCounts[t]) tagCounts[t] = { count: 0, children: {} };
          tagCounts[t].count += 1;
        }
      }
    }
    totalEntries = stats.total;
  } catch {
    /* server offline — sidebar still renders with zeros */
  }

  function kindCount(k: EntryKind): number {
    const row = stats?.by_kind.find((r) => r.kind === k);
    return row?.active ?? 0;
  }

  const totalActive = stats?.active ?? 0;
  const totalDeprecated = stats?.deprecated ?? 0;
  const onList = active === "list";
  const allActive = onList && !activeKind && !activeTag && !showingDeprecated;
  const sortedTags = Object.entries(tagCounts).sort((a, b) => b[1].count - a[1].count);

  return (
    <aside class="sb">
      <div class="sb-brand">
        <div class="sb-brand-mark" aria-hidden="true">
          <i />
          <i />
          <i />
        </div>
        <div class="sb-brand-text">
          <span class="en">KB</span>
          <span class="zh">delta · personal</span>
        </div>
      </div>

      <button
        type="button"
        class="sb-search-trigger"
        data-cmdk-trigger
        aria-label="Open command palette"
      >
        <svg
          width="13"
          height="13"
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
        <span>Search & jump…</span>
        <span class="kbd">⌘K</span>
      </button>

      <a class="sb-new" href="/new">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M12 5v14M5 12h14" />
        </svg>
        <span>New entry</span>
        <span class="kbd">⌘N</span>
      </a>

      <div class="sb-section">
        <Stripe3 vertical={false} /> Library
      </div>
      <nav class="sb-list">
        <a class={`sb-item ${allActive ? "active" : ""}`} href="/">
          <span class="dot" style="background: var(--sakya-fg-tertiary)" />
          <span class="lbl">All entries</span>
          <span class="count">{totalActive}</span>
        </a>
        {KINDS.map((k) => {
          const m = KIND_META[k];
          const on = onList && activeKind === k && !activeTag;
          return (
            <a class={`sb-item ${on ? "active" : ""}`} href={`/?kind=${k}`}>
              <span class={`dot kinddot-${m.cls}`} />
              <span class="lbl">{k}</span>
              <span class="count">{kindCount(k)}</span>
            </a>
          );
        })}
        <a class={`sb-item ${showingDeprecated ? "active" : ""}`} href="/?d=1">
          <span class="dot" style="background: var(--sakya-fg-disabled)" />
          <span class="lbl" style="color: var(--sakya-fg-tertiary)">
            Deprecated
          </span>
          <span class="count">{totalDeprecated}</span>
        </a>
      </nav>

      {sortedTags.length > 0 && (
        <>
          <div class="sb-section">
            <Stripe3 vertical={false} /> Tags
          </div>
          <nav class="sb-list">
            {sortedTags.map(([top, g]) => {
              const topPrefix = `${top}/`;
              const on = onList && activeTag === topPrefix;
              const childs = Object.entries(g.children)
                .sort((a, b) => b[1] - a[1])
                .slice(0, 3);
              return (
                <>
                  <a
                    class={`sb-item sb-tag ${on ? "active" : ""}`}
                    href={`/?tag=${encodeURIComponent(topPrefix)}`}
                  >
                    <span class="lbl">{top}/</span>
                    <span class="count">{g.count}</span>
                  </a>
                  {childs.map(([c, cnt]) => {
                    const fullTag = `${top}/${c}`;
                    const cOn = onList && activeTag === fullTag;
                    return (
                      <a
                        class={`sb-item sb-tag child ${cOn ? "active" : ""}`}
                        href={`/?tag=${encodeURIComponent(fullTag)}`}
                      >
                        <span class="lbl">{c}</span>
                        <span class="count">{cnt}</span>
                      </a>
                    );
                  })}
                </>
              );
            })}
          </nav>
        </>
      )}

      <div class="sb-foot">
        <div class="row">
          <span>entries</span>
          <span>{totalEntries}</span>
        </div>
        <a
          class="row"
          href="/stats"
          style="cursor: default; text-decoration: none; color: inherit;"
        >
          <span>stats</span>
          <span>↗</span>
        </a>
        <div class="row" style="margin-top: 4px; opacity: .7">
          <span>sakya · dorje</span>
          <span>v2.2</span>
        </div>
      </div>
    </aside>
  );
}
