import { PropsWithChildren, createContext, useContext, useState } from 'react';
import { useMediaQuery } from 'react-responsive';

export type OperatingSystem = 'browser' | 'linux' | 'macOS' | 'windows' | 'unknown';

// Platform represents the underlying native layer the app is running on.
// This could be Tauri or web.
export type Platform = {
	platform: 'web' | 'tauri'; // This represents the specific platform implementation
	getThumbnailUrlById: (casId: string) => string;
	getFileUrl: (libraryId: string, locationLocalId: number, filePathId: number) => string;
	openLink: (url: string) => void;
	demoMode?: boolean; // TODO: Remove this in favour of demo mode being handled at the React Query level
	getOs?(): Promise<OperatingSystem>;
	openDirectoryPickerDialog?(): Promise<null | string | string[]>;
	openFilePickerDialog?(): Promise<null | string | string[]>;
	saveFilePickerDialog?(): Promise<string | null>;
	showDevtools?(): void;
	openPath?(path: string): void;
	// Opens a file path with a given ID
	openFilePath?(library: string, id: number): any;
	getFilePathOpenWithApps?(library: string, id: number): any;
	openFilePathWith?(library: string, id: number, appUrl: string): any;
};

// Keep this private and use through helpers below
const context = createContext<Platform>(undefined!);

// is a hook which allows you to fetch information about the current platform from the React context.
export function usePlatform(): Platform {
	const ctx = useContext(context);
	if (!ctx)
		throw new Error(
			"The 'PlatformProvider' has not been mounted above the current 'usePlatform' call."
		);

	return ctx;
}

// provides the platform context to the rest of the app through React context.
// Mount it near the top of your component tree.
export function PlatformProvider({
	platform,
	children
}: PropsWithChildren<{ platform: Platform }>) {
	return <context.Provider value={platform}>{children}</context.Provider>;
}

export const useIsDark = () => {
	const systemPrefersDark = useMediaQuery(
		{
			query: '(prefers-color-scheme: dark)'
		},
		undefined,
		(prefersDark: boolean) => {
			setIsDark(prefersDark);
		}
	);
	const [isDark, setIsDark] = useState(systemPrefersDark);

	return isDark;
};
