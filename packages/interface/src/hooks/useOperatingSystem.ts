import { OperatingSystem, usePlatform } from '@sd/client';
import { useQuery } from '@tanstack/react-query';

export function guessOperatingSystem(): OperatingSystem {
	let os: OperatingSystem = 'unknown';
	if (navigator.userAgent.indexOf('Win') != -1) os = 'windows';
	if (navigator.userAgent.indexOf('Mac') != -1) os = 'macOS';
	if (navigator.userAgent.indexOf('X11') != -1 || navigator.userAgent.indexOf('Linux') != -1)
		os = 'linux';
	return os;
}

// This hook will return the current os we are using. It will guess the OS on first render until Tauri responds with a more accurate answer.
// This means the app can open insanely quickly without any weird layout shift.
// Setting `realOs` to true will return a best guess of the underlying operating system instead of 'browser'.
export function useOperatingSystem(realOs?: boolean): OperatingSystem {
	const platform = usePlatform();
	const { data } = useQuery(
		['_tauri', 'platform'],
		async () => {
			return platform.getOs ? await platform.getOs() : guessOperatingSystem();
		},
		{
			// Here we guess the users operating system from the user agent for the first render.
			initialData: guessOperatingSystem,
			enabled: platform.getOs !== undefined
		}
	);

	return platform.platform === 'web' && !realOs ? 'browser' : data;
}
