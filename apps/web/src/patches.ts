import { WebsocketTransport } from '@rspc/client';

const serverOrigin = import.meta.env.VITE_SDSERVER_ORIGIN || 'localhost:8080';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcTransport = new WebsocketTransport(`ws://${serverOrigin}/rspc/ws`);
