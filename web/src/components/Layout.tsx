import type { Child } from "hono/jsx";
import type { EntryKind } from "../lib/types";
import { Sidebar } from "./Sidebar";

interface Props {
  title?: string;
  active?: "list" | "new" | "stats";
  activeKind?: EntryKind;
  activeTag?: string | null;
  showingDeprecated?: boolean;
  children: Child;
}

export async function Layout(props: Props) {
  const { title = "KB", active, activeKind, activeTag, showingDeprecated, children } = props;
  return (
    <html lang="zh-CN">
      <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>{`${title} · KB`}</title>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="" />
        <link
          href="https://fonts.googleapis.com/css2?family=Geist:wght@300;400;500;600;700&family=Geist+Mono:wght@400;500;600&family=Noto+Serif+SC:wght@500;600&display=swap"
          rel="stylesheet"
        />
        <link rel="stylesheet" href="/styles/sakya-dorje.css" />
        <link rel="stylesheet" href="/styles/kb.css" />
      </head>
      <body>
        <div class="app">
          <Sidebar
            active={active}
            activeKind={activeKind}
            activeTag={activeTag}
            showingDeprecated={showingDeprecated}
          />
          <main class="main">
            <div class="main-inner">{children}</div>
          </main>
        </div>
        <div id="cmdk-root" aria-hidden="true" />
        <script src="/js/app.js" defer />
      </body>
    </html>
  );
}
