import { Ref, useEffect, useRef } from 'react';

export default function useForwardedRef<T>(forwardedRef: Ref<T>) {
	const innerRef = useRef<T>(null);

	useEffect(() => {
		if (!forwardedRef) return;

		if (typeof forwardedRef === 'function') {
			forwardedRef(innerRef.current);
		} else {
			(forwardedRef as React.MutableRefObject<T | null>).current = innerRef.current;
		}
	});

	return innerRef;
}
