import { useCallback, useEffect, useRef } from 'react';
import { EventType, FieldPath, FieldValues, UseFormReturn } from 'react-hook-form';

export function useCallbackToWatchForm<S extends FieldValues>(
	form: UseFormReturn<S, unknown>,
	callback: (
		value: S,
		info: {
			name?: FieldPath<S>;
			type?: EventType;
		}
	) => void | Promise<void>,
	deps: React.DependencyList
) {
	// Create a promise chain to make sure the callback is called in order
	const chain = useRef<Promise<true | void>>(Promise.resolve(true));

	// Disable lint warning because this is just a wrap for useCallback
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const onWatch = useCallback(callback, deps);

	useEffect(() => {
		chain.current = chain.current.then((initCheck) => {
			// If this is the first time, we don't need to wait for a form cahnge
			if (initCheck) onWatch(form.getValues(), {});
		});

		return form.watch((_, info) => {
			chain.current = chain.current.then(() => void onWatch(form.getValues(), info));
		}).unsubscribe;
	}, [form, onWatch]);
}
