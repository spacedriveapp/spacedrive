import { useEffect, useState } from 'react';

// Use a media query to detect light theme changes, because our default theme is dark
const lightMediaQuery = matchMedia('(prefers-color-scheme: light)');

export function useIsDark(): boolean {
	const [isDark, setIsDark] = useState(!lightMediaQuery.matches);

	useEffect(() => {
		const handleChange = () => setIsDark(!lightMediaQuery.matches);
		lightMediaQuery.addEventListener('change', handleChange);
		return () => lightMediaQuery.removeEventListener('change', handleChange);
	}, [setIsDark]);

	return isDark;
}
