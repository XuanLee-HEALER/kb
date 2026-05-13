import { KindBadge } from "../components/KindBadge";
import { KindFieldsForm } from "../components/KindFieldsForm";
import { Layout } from "../components/Layout";
import { Stripe3 } from "../components/Stripe3";
import { KIND_META } from "../lib/kind-meta";
import type { Entry } from "../lib/types";

interface Props {
  entry: Entry | null;
  error: string | null;
  allTags: string[];
}

export function Edit({ entry, error, allTags }: Props) {
  return (
    <Layout title={entry ? `Edit · ${entry.title}` : "Edit"}>
      <div data-page="edit">
        {error && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>Failed to load entry</h3>
            <p>{error}</p>
          </div>
        )}
        {entry && (
          <form action={`/api/entries/${entry.id}/update`} method="post">
            <div class="ph">
              <div class="ph-title">
                <div class="ph-eyebrow">
                  <Stripe3 vertical={false} />
                  <span>edit · 修改</span>
                </div>
                <h1>
                  Editing <span class="mono-sub">{entry.title}</span>
                </h1>
                <div class="ph-sub">Kind is immutable. Changing kind requires a supersede.</div>
              </div>
              <div class="ph-actions">
                <a class="btn ghost" href={`/entry/${entry.id}`}>
                  Cancel
                </a>
                <button type="submit" class="btn primary">
                  Save changes
                  <span style="font-family: var(--font-mono); font-size: 10.5px; opacity: .7; margin-left: 4px">
                    ⌘↵
                  </span>
                </button>
              </div>
            </div>

            <div class="form-grid">
              <div>
                <div class="fld" style="margin-bottom: 18px">
                  <div class="fld-label">
                    <span>kind</span>
                    <span class="hint">immutable · 不可变</span>
                  </div>
                  <div style="display: flex; align-items: center; gap: 10px; padding: 6px 0">
                    <KindBadge kind={entry.kind} />
                    <span style="color: var(--sakya-fg-tertiary); font-family: var(--font-mono); font-size: 12px">
                      {KIND_META[entry.kind].zh} · change requires supersede
                    </span>
                  </div>
                  <input type="hidden" name="kind" value={entry.kind} />
                </div>

                <div class="fld">
                  <div class="fld-label">
                    <span>title</span>
                  </div>
                  <input
                    class="fld-input"
                    name="title"
                    required
                    value={entry.title}
                    style="font-size: 16px"
                  />
                </div>

                <div class="fld">
                  <div class="fld-label">
                    <span>tags</span>
                    <span class="hint">domain/specific convention · 路径风格</span>
                  </div>
                  <div data-tag-input data-all-tags={JSON.stringify(allTags)}>
                    <input type="hidden" name="tags" value={JSON.stringify(entry.tags)} />
                  </div>
                </div>

                <div style="margin: 20px 0 14px; display: flex; align-items: center; gap: 10px; font-family: var(--font-mono); font-size: 10.5px; font-weight: 600; letter-spacing: .12em; text-transform: uppercase; color: var(--sakya-fg-tertiary); white-space: nowrap">
                  <Stripe3 vertical={false} />
                  <span>{entry.kind.toLowerCase()} fields · 字段</span>
                  <span style="flex: 1; height: 1px; background: var(--sakya-border-subtle); margin-left: 6px" />
                </div>

                <KindFieldsForm
                  kind={entry.kind}
                  prefill={entry as unknown as Record<string, unknown>}
                  hiddenByDefault={false}
                />

                <div class="fld" style="margin-top: 10px">
                  <div class="fld-label">
                    <span>body</span>
                    <span class="hint">
                      markdown ·{" "}
                      <code style="font-family: var(--font-mono); color: var(--kind-ps)">
                        [[ULID]]
                      </code>{" "}
                      引用其它条目
                    </span>
                  </div>
                  <textarea class="fld-textarea body" name="body">
                    {entry.body}
                  </textarea>
                </div>

                <div class="form-footer">
                  <div class="hint">
                    <span class="kbd">⌘↵</span> submit&nbsp;&nbsp;<span class="kbd">⎋</span> cancel
                  </div>
                  <div style="display: flex; gap: 8px">
                    <a class="btn ghost" href={`/entry/${entry.id}`}>
                      Cancel
                    </a>
                    <button type="submit" class="btn primary">
                      Save changes
                    </button>
                  </div>
                </div>
              </div>

              <aside class="form-meta-side">
                <h4>
                  <Stripe3 vertical={false} /> Hints
                </h4>
                <p>
                  <strong style="color: var(--sakya-fg-primary)">Why this kind?</strong>
                  <br />
                  <span style="color: var(--sakya-fg-secondary)">
                    {KIND_META[entry.kind].blurb}
                  </span>
                  <br />
                  {KIND_META[entry.kind].blurbZh}
                </p>
                <p>
                  <strong style="color: var(--sakya-fg-primary)">◆ NK fields</strong>
                  <br />
                  <span style="font-family: var(--font-mono); font-size: 11.5px; color: var(--sakya-fg-secondary)">
                    {KIND_META[entry.kind].nkList.join(", ")}
                  </span>
                  <br />
                  These participate in dedup identity. Changes here may trigger duplicates.
                </p>
                <h4 style="margin-top: 4px">Version</h4>
                <p>
                  Saving creates v{entry.version + 1}; v{entry.version} is kept in entry_history.
                </p>
              </aside>
            </div>
          </form>
        )}
      </div>
    </Layout>
  );
}
