import { DependencyList, Dispatch, RefObject, SetStateAction, useCallback, useEffect } from 'react';

export type ResizeRect = Readonly<Omit<DOMRectReadOnly, 'toJSON'>>;
type Cb = (rect: ResizeRect) => void;

const defaultRect: ResizeRect = {
	y: 0,
	x: 0,
	top: 0,
	left: 0,
	right: 0,
	width: 0,
	height: 0,
	bottom: 0
};

const observedElementsCb = new WeakMap<Element, Set<Cb>>();

// Why use a single ResizeObserver instead of one per component?
// https://github.com/WICG/resize-observer/issues/59
const resizeObserver = new ResizeObserver((entries) => {
	for (const entry of entries) {
		const elem = entry.target;
		const cbs = observedElementsCb.get(elem);
		if (cbs) {
			// TODO: contentRect is included in the spec for web compat reasons, and may be deprecated one day
			// Find a way to reconstruct contentRect from the other properties
			// Do not use elem.getBoundingClientRect() as it is very CPU expensive
			for (const cb of cbs) cb(entry.contentRect);
		} else {
			resizeObserver.unobserve(elem);
		}
	}
});

export function useCallbackToWatchResize(
	callback: Cb,
	deps: [RefObject<Element>, ...React.DependencyList]
): void;
export function useCallbackToWatchResize(
	callback: Cb,
	deps: React.DependencyList,
	_ref: RefObject<Element>
): void;
export function useCallbackToWatchResize(
	callback: Cb,
	deps: DependencyList,
	_ref?: RefObject<Element>
) {
	const ref = _ref ?? (deps[0] as RefObject<Element> | undefined);
	if (ref == null) throw new Error('Element not provided');

	// Disable lint warning because this hook is a wrapper for useCallback
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const onResize = useCallback(callback, deps);

	useEffect(() => {
		const elem = ref.current;
		if (elem == null) {
			onResize(defaultRect);
			return;
		}

		const setStates =
			observedElementsCb.get(elem) ?? new Set<Dispatch<SetStateAction<ResizeRect>>>();
		observedElementsCb.set(elem, setStates);
		setStates.add(onResize);

		resizeObserver.observe(elem);

		return () => {
			resizeObserver.unobserve(elem);
			setStates.delete(onResize);
			if (setStates.size === 0) observedElementsCb.delete(elem);
		};
	}, [ref, onResize]);
}
