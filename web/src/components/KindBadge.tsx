import { kindCls } from "../lib/kind-meta";
import type { EntryKind } from "../lib/types";

const svgProps = {
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 2,
  "stroke-linecap": "round" as const,
  "stroke-linejoin": "round" as const,
};

function Glyph({ kind }: { kind: EntryKind }) {
  switch (kind) {
    case "Fact":
      return (
        <svg class="glyph" viewBox="0 0 24 24" {...svgProps}>
          <path d="M5 12l5 5 9-11" />
        </svg>
      );
    case "ProblemSolution":
      return (
        <svg class="glyph" viewBox="0 0 24 24" {...svgProps}>
          <circle cx="12" cy="12" r="9" />
          <path d="M9 9c0-2 1.5-3 3-3s3 1 3 3-3 2-3 4" />
          <circle cx="12" cy="17" r=".5" />
        </svg>
      );
    case "Lesson":
      return (
        <svg class="glyph" viewBox="0 0 24 24" {...svgProps}>
          <path d="M12 3l10 18H2L12 3z" />
          <path d="M12 10v4" />
          <circle cx="12" cy="17" r=".5" />
        </svg>
      );
    case "Decision":
      return (
        <svg class="glyph" viewBox="0 0 24 24" {...svgProps}>
          <path d="M3 12l4-4 4 4-4 4-4-4z" />
          <path d="M13 12l4-4 4 4-4 4-4-4z" />
        </svg>
      );
    case "Heuristic":
      return (
        <svg class="glyph" viewBox="0 0 24 24" {...svgProps}>
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12c4 2 14 2 18 0" />
          <path d="M12 3c2 3 2 15 0 18" />
        </svg>
      );
  }
}

export function KindBadge({ kind }: { kind: EntryKind }) {
  return (
    <span class={`badge icon-lucide kindfg-${kindCls(kind)}`}>
      <Glyph kind={kind} />
      <span>{kind}</span>
    </span>
  );
}
