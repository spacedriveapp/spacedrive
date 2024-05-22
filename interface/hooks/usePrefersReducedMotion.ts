import { useEffect, useState } from 'react';

const QUERY = '(prefers-reduced-motion: no-preference)';

const isRenderingOnServer = typeof window === 'undefined';

const getInitialState = () => {
	// For our initial server render, we won't know if the user
	// prefers reduced motion, but it doesn't matter. This value
	// will be overwritten on the client, before any animations
	// occur.
	return isRenderingOnServer ? true : !window.matchMedia(QUERY).matches;
};

export function usePrefersReducedMotion() {
	const [prefersReducedMotion, setPrefersReducedMotion] = useState(getInitialState);

	useEffect(() => {
		const mediaQueryList = window.matchMedia(QUERY);
		const listener = (event: { matches: any }) => {
			setPrefersReducedMotion(!event.matches);
		};
		if (mediaQueryList.addEventListener) {
			mediaQueryList.addEventListener('change', listener);
		} else {
			mediaQueryList.addListener(listener);
		}
		return () => {
			if (mediaQueryList.removeEventListener) {
				mediaQueryList.removeEventListener('change', listener);
			} else {
				mediaQueryList.removeListener(listener);
			}
		};
	}, []);
	return prefersReducedMotion;
}
