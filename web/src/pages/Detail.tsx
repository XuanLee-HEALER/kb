import { KindBadge } from "../components/KindBadge";
import { KindFieldsDetail } from "../components/KindFieldsDetail";
import { Layout } from "../components/Layout";
import { Stripe3 } from "../components/Stripe3";
import { TagMini } from "../components/TagMini";
import { fmtDateLong } from "../lib/kind-meta";
import type { Entry } from "../lib/types";

interface Props {
  entry: Entry | null;
  error: string | null;
  bodyHtml: string;
  supersedeeTitle?: string;
}

export function Detail({ entry, error, bodyHtml, supersedeeTitle }: Props) {
  return (
    <Layout title={entry?.title ?? "Entry"}>
      <div data-page="detail">
        {error && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>Failed to load entry</h3>
            <p>{error}</p>
          </div>
        )}

        {entry && (
          <div>
            <div class="dh">
              <div class="breadcrumb">
                <a href="/">← library</a>
                <span>/</span>
                <a href={`/?kind=${entry.kind}`}>{entry.kind}</a>
              </div>
              <h1>{entry.title || "(untitled)"}</h1>
              <div class="metarow">
                <KindBadge kind={entry.kind} />
                {entry.deprecated_at && (
                  <span
                    class="kindfg-lesson"
                    style="font-family: var(--font-mono); font-size: 11px; letter-spacing: .04em; text-transform: uppercase"
                  >
                    · deprecated
                  </span>
                )}
                <span>v{entry.version}</span>
                <span class="sep">·</span>
                <span>{fmtDateLong(entry.updated_at)}</span>
                <span class="sep">·</span>
                <span title={entry.id}>
                  id&nbsp;<span style="color: var(--sakya-fg-secondary)">{entry.id}</span>
                </span>
                <span class="sep">·</span>
                <span>
                  source&nbsp;
                  <span style="color: var(--sakya-fg-secondary)">
                    {typeof entry.source === "object" && entry.source
                      ? (entry.source as { type: string }).type
                      : "Human"}
                  </span>
                </span>
              </div>

              {entry.tags.length > 0 && (
                <div class="tagrow">
                  {entry.tags.map((t) => (
                    <TagMini tag={t} />
                  ))}
                </div>
              )}

              {entry.deprecated_at && (
                <div class="deprecated-callout">
                  <strong>⊘ Deprecated</strong>
                  <span>
                    {" "}
                    — on {fmtDateLong(entry.deprecated_at)}.
                    {supersedeeTitle && entry.superseded_by && (
                      <>
                        {" "}
                        Superseded by{" "}
                        <a
                          href={`/entry/${entry.superseded_by}`}
                          style="color: var(--sakya-accent-primary)"
                        >
                          {supersedeeTitle}
                        </a>
                        .
                      </>
                    )}
                    {entry.deprecation_reason && <> Reason: {entry.deprecation_reason}.</>}
                  </span>
                </div>
              )}

              <div class="actions">
                <a class="btn ghost" href="/" title="Back (Esc)">
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
                    <path d="M19 12H5M12 5l-7 7 7 7" />
                  </svg>
                </a>
                {!entry.deprecated_at && (
                  <a class="btn" href={`/edit/${entry.id}`} data-edit-link>
                    <svg
                      width="12"
                      height="12"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <path d="M12 20h9M16.5 3.5a2.121 2.121 0 113 3L7 19l-4 1 1-4z" />
                    </svg>
                    Edit <span class="kbd-inline">E</span>
                  </a>
                )}
                <div style="position: relative" data-menu-host>
                  <button type="button" class="btn ghost" data-menu-trigger aria-label="More">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                      <circle cx="5" cy="12" r="1.6" />
                      <circle cx="12" cy="12" r="1.6" />
                      <circle cx="19" cy="12" r="1.6" />
                    </svg>
                  </button>
                  <div class="menu" data-menu hidden>
                    <div class="menu-item" data-copy-ulid={entry.id} role="button" tabindex={0}>
                      <svg
                        width="12"
                        height="12"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2"
                      >
                        <rect x="9" y="9" width="13" height="13" rx="2" />
                        <path d="M5 15V5a2 2 0 012-2h10" />
                      </svg>
                      Copy ULID
                      <span class="kbd">⌘C</span>
                    </div>
                    {!entry.deprecated_at && (
                      <a class="menu-item" href={`/new?supersedes=${entry.id}`}>
                        <svg
                          width="12"
                          height="12"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="2"
                        >
                          <path d="M12 5v14M5 12l7 7 7-7" />
                        </svg>
                        Supersede with new…
                      </a>
                    )}
                    {!entry.deprecated_at && <div class="menu-divider" />}
                    {!entry.deprecated_at && (
                      <form
                        method="post"
                        action={`/api/entries/${entry.id}/deprecate`}
                        data-confirm="Deprecate this entry? This is irreversible (use Supersede to recover later)."
                        style="margin: 0"
                      >
                        <input type="hidden" name="reason" value="manual deprecation" />
                        <button
                          type="submit"
                          class="menu-item danger"
                          style="width: 100%; text-align: left"
                        >
                          <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                          >
                            <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M6 6l1 14a2 2 0 002 2h6a2 2 0 002-2l1-14" />
                          </svg>
                          Deprecate…
                        </button>
                      </form>
                    )}
                  </div>
                </div>
              </div>
            </div>

            <div class="detail-grid two-col">
              <div class="fields-pane">
                <div class="field-col-h">
                  <Stripe3 vertical={false} />
                  <span>structured fields · 结构化字段</span>
                </div>
                <KindFieldsDetail entry={entry} />
              </div>
              <div class="body-pane">
                <div class="field-col-h">
                  <Stripe3 vertical={false} />
                  <span>body · 正文</span>
                </div>
                {entry.body && entry.body.trim() ? (
                  <div class="prose" dangerouslySetInnerHTML={{ __html: bodyHtml }} />
                ) : (
                  <div class="body-empty">
                    No body — this entry's structured fields say all that needs saying.
                    <br />
                    无正文:结构化字段已包含全部信息。
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </Layout>
  );
}
