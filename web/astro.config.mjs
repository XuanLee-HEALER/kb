// @ts-check
import node from "@astrojs/node";
import { defineConfig } from "astro/config";

export default defineConfig({
  output: "server",
  adapter: node({ mode: "standalone" }),
  server: {
    host: "127.0.0.1",
    port: Number(process.env.PORT ?? 3000),
  },
});
