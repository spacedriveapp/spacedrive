import { useEffect, useReducer } from 'react';
import { useDeviceContext } from 'twrnc';
import { subscribe } from 'valtio';
import { getThemeStore } from '@sd/client';
import { changeTwTheme, tw } from '~/lib/tailwind';

// TODO: Listen for system theme changes if getThemeStore.syncThemeWithSystem is true

export function useTheme() {
	// Enables screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	const [_, forceUpdate] = useReducer((x) => x + 1, 0);

	useEffect(() => {
		const unsubscribe = subscribe(getThemeStore(), () => {
			changeTwTheme(getThemeStore().theme);
			forceUpdate();
		});

		return () => {
			// Cleanup
			unsubscribe();
		};
	}, []);

	// TODO: Listen for system theme changes if getThemeStore.syncThemeWithSystem is true
	// useEffect(() => {

	// 	return () => {
	// 		second;
	// 	};
	// }, []);
}
