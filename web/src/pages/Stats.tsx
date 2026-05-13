import { Layout } from "../components/Layout";
import { Stripe3 } from "../components/Stripe3";
import { KIND_META, KINDS } from "../lib/kind-meta";
import type { Stats as StatsT } from "../lib/types";

interface Props {
  stats: StatsT | null;
  error: string | null;
  activity: number[];
}

export function Stats({ stats, error, activity }: Props) {
  const active = stats?.active ?? 0;
  const total = stats?.total ?? 0;
  const deprecated = stats?.deprecated ?? 0;
  const maxKind = stats ? Math.max(1, ...stats.by_kind.map((r) => r.active + r.deprecated)) : 1;
  const maxAct = Math.max(1, ...activity);

  return (
    <Layout title="Stats" active="stats">
      <div data-page="stats">
        <div class="ph">
          <div class="ph-title">
            <div class="ph-eyebrow">
              <Stripe3 vertical={false} />
              <span>stats · 统计</span>
            </div>
            <h1>Library at a glance</h1>
            <div class="ph-sub">
              Total {total} entries · {active} active · {deprecated} deprecated.
            </div>
          </div>
        </div>

        {error && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>kb-server unreachable</h3>
            <p>{error}</p>
          </div>
        )}

        {stats && (
          <div class="stats-grid">
            <div class="stat-card">
              <h3>
                <Stripe3 vertical={false} /> Totals
              </h3>
              <div class="stat-big">
                <span class="num">{active}</span>
                <span class="sub">active entries · 活跃</span>
              </div>
              <div class="stat-breakdown" style="margin-top: 24px">
                <div class="stat-row" style="border-top: none">
                  <span class="dot" style="background: var(--sakya-accent-primary)" />
                  <span class="name">Active</span>
                  <span class="bar">
                    <span class="fill" style={`width: ${total ? (active / total) * 100 : 0}%`} />
                  </span>
                  <span class="nums">
                    <b>{active}</b> / {total}
                  </span>
                </div>
                <div class="stat-row">
                  <span class="dot" style="background: var(--sakya-bg-surface2)" />
                  <span class="name" style="color: var(--sakya-fg-tertiary)">
                    Deprecated
                  </span>
                  <span class="bar">
                    <span
                      class="fill"
                      style={`width: ${total ? (deprecated / total) * 100 : 0}%; background: var(--sakya-bg-surface2)`}
                    />
                  </span>
                  <span class="nums">
                    <b>{deprecated}</b> / {total}
                  </span>
                </div>
              </div>
            </div>

            <div class="stat-card">
              <h3>
                <Stripe3 vertical={false} /> Per kind
              </h3>
              <div class="stat-breakdown">
                {KINDS.map((k) => {
                  const row = stats.by_kind.find((r) => r.kind === k);
                  const a = row?.active ?? 0;
                  const d = row?.deprecated ?? 0;
                  const m = KIND_META[k];
                  const totK = a + d;
                  return (
                    <div class="stat-row" style={`color: var(--kind-${m.cls})`}>
                      <span class={`dot kinddot-${m.cls}`} />
                      <span class="name">{k}</span>
                      <span class="bar">
                        <span class="fill" style={`width: ${(totK / maxKind) * 100}%`} />
                        {d > 0 && (
                          <span class="dep" style={`right: 0; width: ${(d / maxKind) * 100}%`} />
                        )}
                      </span>
                      <span class="nums">
                        <b>{a}</b>{" "}
                        {d > 0 && <span style="color: var(--sakya-fg-disabled)">+{d}</span>}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>

            <div class="stat-card" style="grid-column: 1 / -1">
              <h3>
                <Stripe3 vertical={false} /> Recent activity · last 30 days
              </h3>
              <div class="stat-timeline">
                {activity.map((v) => {
                  if (v === 0) return <div class="bar" style="height: 3px; opacity: .5" />;
                  const h = (v / maxAct) * 76 + 4;
                  return (
                    <div
                      class="bar"
                      style={`height: ${h}px; background: var(--sakya-accent-primary); opacity: .65`}
                      title={`${v} updates`}
                    />
                  );
                })}
              </div>
              <div style="display: flex; gap: 14px; margin-top: 14px; font-family: var(--font-mono); font-size: 10.5px; color: var(--sakya-fg-tertiary); flex-wrap: wrap">
                <span style="margin-left: auto; color: var(--sakya-fg-disabled)">
                  30 days ago → today
                </span>
              </div>
            </div>
          </div>
        )}
      </div>
    </Layout>
  );
}
