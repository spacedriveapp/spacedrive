import { tauriLink } from '@oscartbeaumont-sd/rspc-tauri/v2';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcLinks = [
	// TODO
	// loggerLink({
	// 	enabled: () => getDebugState().rspcLogger
	// }),
	tauriLink()
];
