import { useCallback, useRef } from 'react';

export type EventPairRegister<T> = (cb: (t: T) => void) => void;

export function useEventPair<T>() {
	const ref = useRef(new EventTarget());

	const trigger = useCallback((t: T) => {
		ref.current.dispatchEvent(new CustomEvent('myEvent', { detail: t }));
	}, []);

	const register: EventPairRegister<T> = useCallback(() => {
		(cb: (t: T) => void) =>
			ref.current.addEventListener('myEvent', (e) => cb((e as CustomEvent<T>).detail));
	}, []);

	return [trigger, register] as const;
}
