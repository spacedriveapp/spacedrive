import { TauriTransport } from '@rspc/tauri';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcTransport = new TauriTransport();
