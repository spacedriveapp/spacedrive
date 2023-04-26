import { useCallback, useEffect, useRef } from 'react';
import { EventType, FieldPath, FieldValues, UseFormReturn } from 'react-hook-form';

export function useCallbackToWatchForm<S extends FieldValues>(
	callback: (
		value: S,
		info: {
			name?: FieldPath<S>;
			type?: EventType;
		}
	) => void | Promise<void>,
	deps: [UseFormReturn<S, unknown>, ...React.DependencyList]
): void;

export function useCallbackToWatchForm<S extends FieldValues>(
	callback: (
		value: S,
		info: {
			name?: FieldPath<S>;
			type?: EventType;
		}
	) => void | Promise<void>,
	deps: React.DependencyList,
	form: UseFormReturn<S, unknown>
): void;

export function useCallbackToWatchForm<S extends FieldValues>(
	callback: (
		value: S,
		info: {
			name?: FieldPath<S>;
			type?: EventType;
		}
	) => void | Promise<void>,
	deps: React.DependencyList,
	form?: UseFormReturn<S, unknown>
): void {
	// Create a promise chain to make sure the callback is called in order
	const chain = useRef<Promise<true | void>>(Promise.resolve(true));
	if (form == null) form = deps[0] as UseFormReturn<S, unknown>;
	if (form == null) throw new Error('Form is not provided');
	const { getValues, watch } = form;

	// Disable lint warning because this is hook is a wrapper for useCallback
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const onWatch = useCallback(callback, deps);

	useEffect(() => {
		chain.current = chain.current.then((initCheck) => {
			// If this is the first time, we don't need to wait for a form cahnge
			if (initCheck) onWatch(getValues(), {});
		});

		return watch((_, info) => {
			chain.current = chain.current.then(() => void onWatch(getValues(), info));
		}).unsubscribe;
	}, [watch, onWatch, getValues]);
}
