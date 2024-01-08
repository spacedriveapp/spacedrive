import { useEffect, useRef, useState } from 'react';
import { createReaction, createRoot } from 'solid-js';

// A version of `react-solid-state`'s method that works with newer React versions.
// https://github.com/solidjs/react-solid-state/issues/4
export function useObserver<T>(fn: () => T) {
	const [_, setTick] = useState(0);
	const state = useRef({
		onUpdate: () => {},
		// An really ugly workaround for React `StrictMode`'s double firing of `useEffect`.
		doneFirstFire: false
	});
	const reaction = useRef<{ dispose: () => void; track: (fn: () => void) => void }>();
	if (!reaction.current) {
		reaction.current = createRoot((dispose) => ({
			dispose,
			track: createReaction(() => state.current.onUpdate())
		}));
	}

	useEffect(() => {
		// We set this after a `useEffect` to ensure we don't trigger an update prior to mount
		// cause that makes React madge.
		state.current.onUpdate = () => setTick((t) => t + 1);
		state.current.doneFirstFire = true;

		return () => {
			if (!state.current.doneFirstFire) {
				reaction.current?.dispose();
				reaction.current = undefined;
			}
		};
	}, []);

	let rendering!: T;
	reaction.current.track(() => (rendering = fn()));
	return rendering;
}
