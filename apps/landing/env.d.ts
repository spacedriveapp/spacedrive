/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SDWEB_BASE_URL: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
