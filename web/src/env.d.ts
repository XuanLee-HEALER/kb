/// <reference types="astro/client" />

interface ImportMetaEnv {
  readonly KB_URL: string;
  readonly KB_TOKEN?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
