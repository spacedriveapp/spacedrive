/// <reference types="vite/client" />

interface ImportMetaEnv {
	readonly VITE_SDSERVER_ORIGIN: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
