import { PropsWithChildren, createContext, useContext } from 'react';

export type OperatingSystem = 'browser' | 'linux' | 'macOS' | 'windows' | 'unknown';

// Platform represents the underlying native layer the app is running on.
// This could be Tauri or web.
export type Platform = {
	platform: 'web' | 'tauri'; // This represents the specific platform implementation
	getThumbnailUrlById: (casId: string) => string;
	demoMode?: boolean; // TODO: Remove this in favour of demo mode being handled at the React Query level
	getOs?(): Promise<OperatingSystem>;
	openFilePickerDialog?(): Promise<null | string | string[]>;
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
