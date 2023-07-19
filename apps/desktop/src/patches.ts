import { tauriLink } from '@rspc/tauri';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcLinks = [tauriLink()];
