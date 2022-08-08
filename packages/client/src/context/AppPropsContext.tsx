import { createContext } from 'react';

export const AppPropsContext = createContext<AppProps | null>(null);

export type Platform = 'browser' | 'macOS' | 'windows' | 'linux';
export type CdnUrl = 'internal' | string;

export interface AppProps {
	platform: Platform;
	data_path?: string;
	cdn_url?: CdnUrl;
	getThumbnailUrlById: (cas_id: string) => string;
	openDialog: (options: { directory?: boolean }) => Promise<string | string[] | null>;
	onClose?: () => void;
	onMinimize?: () => void;
	onFullscreen?: () => void;
	onOpen?: (path: string) => void;
	isFocused?: boolean;
	demoMode?: boolean;
}
