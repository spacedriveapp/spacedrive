import { useEffect, useState } from 'react';
import { useThemeStore } from '@sd/client';

//this hook is being used for some ui elements & icons that need to be inverted

export function useIsDark(): boolean {
	const themeStore = useThemeStore();
	const [isDark, setIsDark] = useState(themeStore.theme === 'dark');

	useEffect(() => {
		if (themeStore.syncThemeWithSystem) {
			if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
				setIsDark(true);
			} else setIsDark(false);
		} else {
			if (themeStore.theme === 'dark') {
				setIsDark(true);
			} else if (themeStore.theme === 'vanilla') {
				setIsDark(false);
			}
		}
	}, [setIsDark, themeStore]);

	return isDark;
}
