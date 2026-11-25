import { useEffect, useSyncExternalStore } from 'react';
import { useCoreQuery, useSpacedriveClient } from '../contexts/SpacedriveContext';

const THEME_CLASS_MAP: Record<string, string> = {
	light: 'vanilla-theme',
	dark: '', // default, no class needed
	midnight: 'midnight-theme',
	noir: 'noir-theme',
	slate: 'slate-theme',
	nord: 'nord-theme',
	mocha: 'mocha-theme',
};

export function applyTheme(theme: string) {
	let resolvedTheme = theme;

	// Handle system theme
	if (theme === 'system') {
		const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
		resolvedTheme = prefersDark ? 'dark' : 'light';
	}

	// Apply theme class to document
	const themeClass = THEME_CLASS_MAP[resolvedTheme];

	// Remove all theme classes
	document.documentElement.classList.remove(
		'vanilla-theme',
		'midnight-theme',
		'noir-theme',
		'slate-theme',
		'nord-theme',
		'mocha-theme'
	);

	// Add the new theme class if it's not dark (dark is default)
	if (themeClass) {
		document.documentElement.classList.add(themeClass);
	}
}

function subscribeToThemeClass(onChange: () => void) {
	const observer = new MutationObserver(onChange);
	observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
	return () => observer.disconnect();
}

/** Whether the light theme is applied now, following `applyTheme` as it swaps the class. */
export function useIsLightTheme(): boolean {
	return useSyncExternalStore(subscribeToThemeClass, () =>
		document.documentElement.classList.contains('vanilla-theme')
	);
}

export function useTheme() {
	const client = useSpacedriveClient();
	const { data: config, refetch } = useCoreQuery({ type: 'config.app.get', input: null as any });

	// Apply theme on mount and when config changes
	useEffect(() => {
		const theme = config?.preferences?.theme || 'system';
		applyTheme(theme);
	}, [config?.preferences?.theme]);

	// Listen for config change events from Rust (following useJobs pattern)
	useEffect(() => {
		if (!client) return;

		console.log('[useTheme] Setting up ConfigChanged subscription');
		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: any) => {
			if ('ConfigChanged' in event) {
				console.log('[useTheme] Received ConfigChanged event:', event.ConfigChanged);
				const configEvent = event.ConfigChanged;
				if (configEvent?.field === 'theme') {
					// Refetch config to get the new theme value
					refetch().then((result) => {
						if (result.data?.preferences?.theme) {
							applyTheme(result.data.preferences.theme);
						}
					});
				}
			}
		};

		const filter = {
			event_types: ['ConfigChanged']
		};

		client.subscribeFiltered(filter, handleEvent).then((unsub) => {
			console.log('[useTheme] ConfigChanged subscription created');
			if (isCancelled) {
				console.log('[useTheme] Subscription was cancelled during setup, cleaning up');
				unsub();
			} else {
				unsubscribe = unsub;
			}
		});

		return () => {
			console.log('[useTheme] Cleaning up ConfigChanged subscription');
			isCancelled = true;
			unsubscribe?.();
		};
	}, [client]);

	// Listen for system theme changes when theme is set to "system"
	useEffect(() => {
		const theme = config?.preferences?.theme || 'system';

		if (theme !== 'system') return;

		const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

		const handleChange = (e: MediaQueryListEvent) => {
			const resolvedTheme = e.matches ? 'dark' : 'light';
			applyTheme(resolvedTheme);
		};

		mediaQuery.addEventListener('change', handleChange);

		return () => mediaQuery.removeEventListener('change', handleChange);
	}, [config?.preferences?.theme]);
}
