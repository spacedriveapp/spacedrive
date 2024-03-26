import { useCallback, useEffect, useRef } from 'react';

// Utility helper for random number generation
const random = (min: number, max: number) => Math.floor(Math.random() * (max - min)) + min;

export const useRandomInterval = (
	callback: () => void,
	minDelay: number | null,
	maxDelay: number | null
) => {
	const timeoutId = useRef<number | null>(null);
	const savedCallback = useRef(callback);

	useEffect(() => {
		savedCallback.current = callback;
	}, [callback]);

	useEffect(() => {
		const isEnabled = typeof minDelay === 'number' && typeof maxDelay === 'number';
		if (isEnabled) {
			const handleTick = () => {
				const nextTickAt = random(minDelay, maxDelay);
				timeoutId.current = window.setTimeout(() => {
					savedCallback.current();
					handleTick();
				}, nextTickAt);
			};
			handleTick();
		}
		return () => window.clearTimeout(timeoutId.current!);
	}, [minDelay, maxDelay]);

	const cancel = useCallback(function () {
		window.clearTimeout(timeoutId.current!);
	}, []);

	return cancel;
};
