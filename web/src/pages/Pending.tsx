import { Layout } from "../components/Layout";
import { Stripe3 } from "../components/Stripe3";
import { fmtDate } from "../lib/kind-meta";
import type { Candidate } from "../lib/types";

interface Props {
  candidates: Candidate[];
  error: string | null;
}

// Try to surface a kind_hint from candidate.content. The sediment hook prepends
// "[kind_hint: <Kind>]" as the first line. If absent we just return null.
function extractKindHint(content: string): string | null {
  const m = content.match(/\[kind_hint:\s*([A-Za-z]+)\s*\]/);
  return m && m[1] ? m[1] : null;
}

// Same idea for the "[why: ...]" line.
function extractWhy(content: string): string | null {
  const m = content.match(/\[why:\s*([^\]]+)\]/);
  return m && m[1] ? m[1].trim() : null;
}

function previewBody(content: string): string {
  // Drop our own [kind_hint] [why] [from] header lines; show the next ~240 chars.
  const lines = content.split("\n");
  const start = lines.findIndex(
    (l) =>
      l.trim() !== "" &&
      !l.startsWith("[kind_hint:") &&
      !l.startsWith("[why:") &&
      !l.startsWith("[from:"),
  );
  const body = start >= 0 ? lines.slice(start).join("\n") : content;
  const trimmed = body.replace(/\s+/g, " ").trim();
  return trimmed.length > 240 ? `${trimmed.slice(0, 240)}…` : trimmed;
}

function sourceLabel(src: Candidate["source"]): string {
  if (src.type === "Human") return "Human";
  if (src.type === "ClaudeCode") {
    const cwd = src.cwd || "";
    const proj = src.project || "";
    return cwd ? `ClaudeCode · ${cwd.split("/").slice(-2).join("/")}` : `ClaudeCode · ${proj}`;
  }
  if (src.type === "Imported") return `Imported · ${src.from}`;
  return "(unknown)";
}

export function Pending({ candidates, error }: Props) {
  return (
    <Layout title="Pending pool" active="pool" showingPool>
      <div data-page="pool">
        <div class="ph">
          <div class="ph-title">
            <div class="ph-eyebrow">
              <Stripe3 vertical={false} />
              <span>pool · 候选池</span>
            </div>
            <h1>Pending candidates</h1>
            <div class="ph-sub">
              {candidates.length} raw {candidates.length === 1 ? "candidate" : "candidates"}{" "}
              awaiting promote / discard. Sediment hook deposits these on PreCompact / SessionEnd;
              review with{" "}
              <code style="font-family: var(--font-mono); color: var(--sakya-accent-primary)">
                mcp__kb__promote_candidate
              </code>{" "}
              in a Claude Code session.
            </div>
          </div>
        </div>

        {error && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>kb-server unreachable</h3>
            <p style="max-width: 380px; margin-top: 8px;">{error}</p>
          </div>
        )}

        {!error && candidates.length === 0 && (
          <div class="empty">
            <Stripe3 vertical={false} />
            <h3>Pool is empty.</h3>
            <p style="max-width: 380px; margin-top: 8px;">
              The sediment hook hasn't dropped anything in here yet — either the hook isn't
              installed, or this session's transcripts had no qualifying signal.
              <br />
              <span style="color: var(--sakya-fg-disabled)">
                候选池为空。Hook 还没装,或还没碰到值得记的瞬间。
              </span>
            </p>
          </div>
        )}

        {!error && candidates.length > 0 && (
          <div class="lst comfortable">
            {candidates.map((c) => {
              const kind = extractKindHint(c.content);
              const why = extractWhy(c.content);
              return (
                <div class="row" style="cursor: default">
                  <div class="title-line">
                    {kind && (
                      <span
                        class={`badge kindfg-${kind.toLowerCase()}`}
                        style="font-family: var(--font-mono); font-size: 11px; padding: 2px 8px; border-radius: 4px; border: 1px solid var(--sakya-border-secondary)"
                      >
                        {kind}
                      </span>
                    )}
                    <span class="title">{why || "(no hint)"}</span>
                  </div>
                  <div class="meta">
                    <span>{fmtDate(c.created_at)}</span>
                    <span style="color: var(--sakya-fg-disabled); font-size: 10px">
                      id {c.id.slice(0, 8)} · {sourceLabel(c.source)}
                    </span>
                  </div>
                  <div class="summary">{previewBody(c.content)}</div>
                  <div
                    class="tags"
                    style="display: flex; gap: 8px; margin-top: 6px; font-family: var(--font-mono); font-size: 11px"
                  >
                    <form
                      method="post"
                      action={`/api/candidates/${c.id}/discard`}
                      style="margin: 0"
                    >
                      <button
                        type="submit"
                        class="chip"
                        style="cursor: pointer; background: none; border: 1px dashed var(--sakya-fg-disabled); color: var(--sakya-fg-tertiary)"
                        title="Discard this candidate (cannot be undone)"
                      >
                        discard
                      </button>
                    </form>
                    <span style="color: var(--sakya-fg-disabled); font-size: 10px; align-self: center">
                      promote: open a Claude Code session and call mcp__kb__promote_candidate
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </Layout>
  );
}
