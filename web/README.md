# web

Hono on Bun · server-rendered TSX · zero client framework.

## Layout

```
src/
  server.tsx             Hono app — pages + api routes + static
  components/            shared TSX (Layout, Sidebar, KindBadge, …)
  pages/                 one .tsx per route (List, Detail, New, Edit, Stats, Duplicates)
  lib/                   kb-client / types / kind-meta / form / markdown
  scripts/app.ts         client-side JS (Cmd-K palette, tag chip input, etc.)
public/
  styles/{sakya-dorje,kb}.css   served at /styles/*
  js/app.js              bundled from scripts/app.ts, served at /js/app.js
```

## Commands

| | what |
|---|---|
| `bun run dev`           | build client bundle once, then `bun --hot src/server.tsx` |
| `bun run dev:client`    | watch + rebuild the client bundle |
| `bun run build`         | client bundle + `tsc --noEmit` |
| `bun run start`         | production: `bun src/server.tsx` |
| `bun run check`         | typecheck + biome |
| `bun run lint:fix`      | biome auto-fix |

## Env

`KB_URL` (default `http://127.0.0.1:5100`), `KB_TOKEN`, `PORT` (default `5101`).
