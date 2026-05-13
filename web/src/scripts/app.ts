// Client-side interaction layer.
//
// Mounted from <script> in Base.astro. Handles:
//   • global keyboard shortcuts (⌘K, ⌘N, /, e on detail, Esc)
//   • Cmd-K palette (search + jump-to-ULID + new + stats + kind filter)
//   • form: kind picker / segmented control swap which field group is visible
//   • form: tag chip input
//   • detail: ⋯ menu toggle, click-outside to close

// ─────────────────────────────────────────────────────────────────────────
// shared helpers

type SearchHit = {
  id: string;
  kind: string;
  title: string;
  summary_line: string;
  tags: string[];
  updated_at: string;
  deprecated_at?: string | null;
};

const KIND_CLS: Record<string, string> = {
  Fact: "fact",
  ProblemSolution: "ps",
  Lesson: "lesson",
  Decision: "decision",
  Heuristic: "heuristic",
};

const KIND_ZH: Record<string, string> = {
  Fact: "事实",
  ProblemSolution: "问题方案",
  Lesson: "教训",
  Decision: "决策",
  Heuristic: "启发法",
};

const KINDS = ["Fact", "ProblemSolution", "Lesson", "Decision", "Heuristic"] as const;

let CACHED_HITS: SearchHit[] | null = null;
let CACHED_AT = 0;
const CACHE_TTL_MS = 30_000;

async function getHits(): Promise<SearchHit[]> {
  const now = Date.now();
  if (CACHED_HITS && now - CACHED_AT < CACHE_TTL_MS) return CACHED_HITS;
  try {
    const res = await fetch("/api/search?limit=300");
    if (!res.ok) return [];
    const hits = (await res.json()) as SearchHit[];
    CACHED_HITS = hits;
    CACHED_AT = now;
    return hits;
  } catch {
    return [];
  }
}

// ─────────────────────────────────────────────────────────────────────────
// Cmd-K palette

const ULID_RE = /^[0-9A-HJKMNP-TV-Z]{6,26}$/i;

class CmdKPalette {
  private root: HTMLElement;
  private input!: HTMLInputElement;
  private body!: HTMLElement;
  private q = "";
  private idx = 0;
  private hits: SearchHit[] = [];
  private items: Array<{
    section: string;
    kind: "action" | "filter" | "entry" | "empty";
    id: string;
    title: string;
    meta: string;
    cls?: string;
    icon?: string;
    href?: string;
  }> = [];

  constructor(root: HTMLElement) {
    this.root = root;
  }

  async open(): Promise<void> {
    if (this.root.firstChild) return;
    this.root.innerHTML = this.template();
    this.input = this.root.querySelector("input")!;
    this.body = this.root.querySelector(".cmdk-body")!;
    this.input.addEventListener("input", () => {
      this.q = this.input.value;
      this.idx = 0;
      this.render();
    });
    this.input.addEventListener("keydown", (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        this.close();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        this.idx = Math.min(this.idx + 1, this.items.length - 1);
        this.render();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        this.idx = Math.max(this.idx - 1, 0);
        this.render();
      } else if (e.key === "Enter") {
        e.preventDefault();
        this.activate(this.items[this.idx]);
      }
    });
    const overlay = this.root.querySelector(".cmdk-overlay")!;
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) this.close();
    });
    this.input.focus();
    this.hits = await getHits();
    this.render();
  }

  close(): void {
    this.root.innerHTML = "";
  }

  isOpen(): boolean {
    return this.root.firstChild != null;
  }

  private template(): string {
    return `
<div class="cmdk-overlay">
  <div class="cmdk" role="dialog" aria-label="Command palette">
    <div class="cmdk-head">
      <span class="stripe3" aria-hidden="true"><i></i><i></i><i></i></span>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="7"></circle><path d="m21 21-4.3-4.3"></path>
      </svg>
      <input type="text" placeholder='Search · jump to ULID · "new", "stats", kind name…' />
      <span class="esc">esc</span>
    </div>
    <div class="cmdk-body"></div>
    <div class="cmdk-foot">
      <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
      <span><kbd>↵</kbd> open</span>
      <span><kbd>⎋</kbd> close</span>
      <span style="margin-left: auto; color: var(--sakya-fg-disabled)">libsimple · bm25 · 也可粘贴 ULID 直跳</span>
    </div>
  </div>
</div>`;
  }

  private compute(): typeof this.items {
    const q = this.q.trim();
    const lq = q.toLowerCase();
    const out: typeof this.items = [];

    if (!q || "new".includes(lq)) {
      out.push({
        section: "actions",
        kind: "action",
        id: "new",
        icon: "+",
        title: "New entry",
        meta: "⌘N",
        href: "/new",
      });
    }
    if (!q || "stats".includes(lq)) {
      out.push({
        section: "actions",
        kind: "action",
        id: "stats",
        icon: "∑",
        title: "Stats",
        meta: "",
        href: "/stats",
      });
    }

    for (const k of KINDS) {
      if (q && k.toLowerCase().startsWith(lq)) {
        out.push({
          section: "filters",
          kind: "filter",
          id: `filter:${k}`,
          cls: KIND_CLS[k],
          title: `Filter: ${k}`,
          meta: KIND_ZH[k] ?? "",
          href: `/?kind=${k}`,
        });
      }
    }

    let pool = this.hits;
    const looksLikeUlid = ULID_RE.test(q);
    if (q) {
      if (looksLikeUlid) {
        const upper = q.toUpperCase();
        pool = pool.filter((e) => e.id.startsWith(upper));
      } else {
        pool = pool.filter(
          (e) =>
            e.title.toLowerCase().includes(lq) ||
            (e.summary_line || "").toLowerCase().includes(lq) ||
            e.tags.some((t) => t.toLowerCase().includes(lq)),
        );
      }
    }
    for (const e of pool.slice(0, 10)) {
      out.push({
        section: "entries",
        kind: "entry",
        id: e.id,
        cls: KIND_CLS[e.kind] ?? "",
        title: e.title,
        meta: e.id.slice(0, 10) + "…",
        href: `/entry/${e.id}`,
      });
    }

    if (q && pool.length === 0 && !looksLikeUlid) {
      out.push({
        section: "no-results",
        kind: "empty",
        id: "empty",
        title: `No matches for "${q}"`,
        meta: "try a tag or kind name",
      });
    }
    return out;
  }

  private render(): void {
    this.items = this.compute();
    if (this.idx >= this.items.length) this.idx = Math.max(0, this.items.length - 1);

    const sectionLabel: Record<string, string> = {
      actions: "actions",
      filters: "filters",
      entries: "entries",
    };

    let html = "";
    let lastSection = "";
    this.items.forEach((it, i) => {
      if (it.section !== lastSection) {
        if (sectionLabel[it.section]) {
          html += `<div class="cmdk-section">${sectionLabel[it.section]}</div>`;
        }
        lastSection = it.section;
      }
      const on = i === this.idx ? "on" : "";
      let glyph: string;
      if (it.kind === "action") {
        glyph = `<span class="dot" style="background: var(--sakya-accent-primary); display:inline-flex; align-items:center; justify-content:center; font-family: var(--font-mono); color:#1a1605; font-size:9px; font-weight:700">${it.icon}</span>`;
      } else if (it.kind === "empty") {
        glyph = `<span class="dot" style="background: transparent"></span>`;
      } else {
        glyph = `<span class="dot kinddot-${it.cls}"></span>`;
      }
      const arrow = it.kind === "entry" ? '<span class="arrow">↗</span>' : "";
      html += `<div class="cmdk-item ${on}" data-idx="${i}" role="button" tabindex="0">
        ${glyph}
        <span class="title">${escapeHtml(it.title)}</span>
        ${arrow}
        <span class="meta">${escapeHtml(it.meta)}</span>
      </div>`;
    });
    this.body.innerHTML = html;
    this.body.querySelectorAll<HTMLElement>(".cmdk-item").forEach((el) => {
      const i = Number(el.dataset.idx);
      el.addEventListener("mouseenter", () => {
        this.idx = i;
        this.body
          .querySelectorAll(".cmdk-item")
          .forEach((x, j) => x.classList.toggle("on", j === i));
      });
      el.addEventListener("click", () => {
        this.activate(this.items[i]);
      });
    });
    const onEl = this.body.querySelector(".cmdk-item.on");
    if (onEl) (onEl as HTMLElement).scrollIntoView({ block: "nearest" });
  }

  private activate(it: (typeof this.items)[number] | undefined): void {
    if (!it || it.kind === "empty") return;
    if (it.href) {
      this.close();
      window.location.href = it.href;
    }
  }
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// ─────────────────────────────────────────────────────────────────────────
// Tag chip input
// Wires up any <div data-tag-input> with two inputs inside:
//   - .tag-input        the visible chip container
//   - input[name=tags]  the hidden real form input

function initTagInputs(): void {
  document.querySelectorAll<HTMLElement>("[data-tag-input]").forEach((host) => {
    const hidden = host.querySelector<HTMLInputElement>('input[type="hidden"][name="tags"]')!;
    const allTags = JSON.parse(host.dataset.allTags ?? "[]") as string[];
    let tags: string[] = JSON.parse(hidden.value || "[]");

    function render(): void {
      const chips = tags
        .map((t) => {
          const parts = t
            .split("/")
            .map((p, i) =>
              i > 0
                ? `<span style="color: var(--sakya-fg-disabled)">/</span>${escapeHtml(p)}`
                : escapeHtml(p),
            )
            .join("");
          return `<span class="chip-tag">${parts}<span class="x" data-rm="${escapeHtml(t)}" role="button" tabindex="0">✕</span></span>`;
        })
        .join("");
      ti.innerHTML = `${chips}<input type="text" placeholder="${tags.length === 0 ? "rust/tokio, network/wireguard…" : ""}" />`;
      const input = ti.querySelector<HTMLInputElement>("input")!;
      input.focus();
      input.addEventListener("keydown", onKey);
      input.addEventListener("input", onInput);
      ti.querySelectorAll<HTMLElement>("[data-rm]").forEach((el) =>
        el.addEventListener("click", (e) => {
          e.stopPropagation();
          tags = tags.filter((t) => t !== el.dataset.rm);
          syncHidden();
          render();
        }),
      );
      hidden.value = JSON.stringify(tags);
    }

    const ti = document.createElement("div");
    ti.className = "tag-input";
    host.appendChild(ti);

    const sugg = document.createElement("div");
    sugg.className = "tag-autocomplete";
    host.appendChild(sugg);

    function syncHidden(): void {
      hidden.value = JSON.stringify(tags);
    }

    function commit(raw: string): void {
      const v = raw.trim().replace(/,$/, "");
      if (!v || tags.includes(v)) return;
      tags.push(v);
      syncHidden();
      render();
    }

    function onInput(e: Event): void {
      const v = (e.target as HTMLInputElement).value.trim();
      if (!v) {
        sugg.innerHTML = "";
        return;
      }
      const matches = allTags
        .filter((t) => t.toLowerCase().includes(v.toLowerCase()) && !tags.includes(t))
        .slice(0, 6);
      if (matches.length === 0) {
        sugg.innerHTML = "";
        return;
      }
      sugg.innerHTML = matches
        .map(
          (t) =>
            `<span class="sugg" data-tag="${escapeHtml(t)}" role="button" tabindex="0">${escapeHtml(t)}</span>`,
        )
        .join("");
      sugg.querySelectorAll<HTMLElement>("[data-tag]").forEach((el) => {
        el.addEventListener("click", () => {
          commit(el.dataset.tag!);
          sugg.innerHTML = "";
        });
      });
    }

    function onKey(e: KeyboardEvent): void {
      const input = e.target as HTMLInputElement;
      if (e.key === "Enter" || e.key === "," || e.key === " ") {
        e.preventDefault();
        commit(input.value);
        sugg.innerHTML = "";
      } else if (e.key === "Backspace" && input.value === "" && tags.length) {
        tags = tags.slice(0, -1);
        syncHidden();
        render();
      }
    }

    render();
  });
}

// ─────────────────────────────────────────────────────────────────────────
// Kind picker / segmented control — swap visible field group on the form

function initKindSwitcher(): void {
  document.querySelectorAll<HTMLElement>("[data-kind-switcher]").forEach((host) => {
    const hidden = host.querySelector<HTMLInputElement>('input[type="hidden"][name="kind"]')!;
    const items = host.querySelectorAll<HTMLElement>("[data-kind-option]");
    const formRoot = host.closest<HTMLElement>("form") ?? document;
    const fieldGroups = formRoot.querySelectorAll<HTMLElement>("[data-kind-fields]");

    function select(kind: string): void {
      hidden.value = kind;
      items.forEach((el) => {
        el.classList.toggle("on", el.dataset.kindOption === kind);
        if (el.dataset.kindOption === kind) {
          el.setAttribute("aria-selected", "true");
        } else {
          el.removeAttribute("aria-selected");
        }
      });
      fieldGroups.forEach((g) => {
        const show = g.dataset.kindFields === kind;
        g.hidden = !show;
      });
      // Update hints sidebar if present
      const hintsBlurb = document.querySelector<HTMLElement>("[data-hint-blurb]");
      const hintsBlurbZh = document.querySelector<HTMLElement>("[data-hint-blurb-zh]");
      const hintsNk = document.querySelector<HTMLElement>("[data-hint-nk]");
      const meta = (host.dataset.meta && JSON.parse(host.dataset.meta)) || {};
      if (hintsBlurb && meta[kind]) hintsBlurb.textContent = meta[kind].blurb;
      if (hintsBlurbZh && meta[kind]) hintsBlurbZh.textContent = meta[kind].blurbZh;
      if (hintsNk && meta[kind]) hintsNk.textContent = meta[kind].nkList.join(", ");
      // Update fields label
      const lbl = document.querySelector<HTMLElement>("[data-kind-fields-label]");
      if (lbl) lbl.textContent = kind.toLowerCase() + " fields · 字段";
    }

    items.forEach((el) => {
      el.addEventListener("click", () => select(el.dataset.kindOption!));
      el.addEventListener("keydown", (e: KeyboardEvent) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          select(el.dataset.kindOption!);
        }
      });
    });

    select(hidden.value || "ProblemSolution");
  });
}

// ─────────────────────────────────────────────────────────────────────────
// Detail page ⋯ menu

function initMenus(): void {
  document.querySelectorAll<HTMLElement>("[data-menu-host]").forEach((host) => {
    const trigger = host.querySelector<HTMLElement>("[data-menu-trigger]")!;
    const menu = host.querySelector<HTMLElement>("[data-menu]")!;
    menu.hidden = true;
    trigger.addEventListener("click", (e) => {
      e.stopPropagation();
      menu.hidden = !menu.hidden;
    });
    document.addEventListener("click", (e) => {
      if (!host.contains(e.target as Node)) menu.hidden = true;
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") menu.hidden = true;
    });
  });
}

// ─────────────────────────────────────────────────────────────────────────
// global keyboard

function initGlobalKeys(palette: CmdKPalette): void {
  document.addEventListener("keydown", (e) => {
    const t = e.target as HTMLElement | null;
    const inField = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    const mod = e.metaKey || e.ctrlKey;

    if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      if (palette.isOpen()) palette.close();
      else palette.open();
      return;
    }
    if (mod && e.key.toLowerCase() === "n" && !inField) {
      e.preventDefault();
      window.location.href = "/new";
      return;
    }
    if (e.key === "/" && !inField) {
      e.preventDefault();
      palette.open();
      return;
    }
    if (e.key === "e" && !inField && document.body.dataset.page === "detail") {
      const editLink = document.querySelector<HTMLAnchorElement>("[data-edit-link]");
      if (editLink) {
        window.location.href = editLink.href;
      }
      return;
    }
    if (e.key === "Escape") {
      // close any open menu/sheet; palette closes itself via its input keydown.
      document.querySelectorAll<HTMLElement>("[data-menu]").forEach((m) => {
        m.hidden = true;
      });
    }
  });
}

// ─────────────────────────────────────────────────────────────────────────
// boot

document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll<HTMLElement>("[data-stop-propagation]").forEach((el) => {
    el.addEventListener("click", (e) => e.stopPropagation());
  });
  const root = document.getElementById("cmdk-root");
  if (!root) return;
  const palette = new CmdKPalette(root);
  const trigger = document.querySelector<HTMLElement>("[data-cmdk-trigger]");
  if (trigger) trigger.addEventListener("click", () => palette.open());
  initTagInputs();
  initKindSwitcher();
  initMenus();
  initGlobalKeys(palette);
});
