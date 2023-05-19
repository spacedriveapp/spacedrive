import { useCallback, useEffect, useRef } from 'react';
import { EventType, FieldPath, FieldValues, UseFormReturn } from 'react-hook-form';

const noop = () => {};

type Cb<S extends FieldValues> = (
	value: S,
	info: {
		name?: FieldPath<S>;
		type?: EventType;
	}
) => void | Promise<void>;

export function useCallbackToWatchForm<S extends FieldValues>(
	callback: Cb<S>,
	deps: [UseFormReturn<S, unknown>, ...React.DependencyList]
): void;

export function useCallbackToWatchForm<S extends FieldValues>(
	callback: Cb<S>,
	deps: React.DependencyList,
	form: UseFormReturn<S, unknown>
): void;

/**
 * This hook is an async friendly wrapper for useCallback that enables reacting to any form changes.
 *
 * The callback will be called on the first render, and whenever the form changes, with the current
 * form values and the event info regarding the change (empty on first render). If the callback is
 * async, or returns a promise, it will wait for the previous callback to finish before executing
 * the next one. Any errors thrown by the callback will be ignored.
 *
 * @param callback - Callback to be called when form changes
 * @param deps - Dependency list for the callback
 * @param form - Form to watch. If not provided, it will be taken from the first element of the dependency list
 */
export function useCallbackToWatchForm<S extends FieldValues>(
	callback: Cb<S>,
	deps: React.DependencyList,
	form?: UseFormReturn<S, unknown>
): void {
	if (form == null) form = deps[0] as UseFormReturn<S, unknown>;
	if (form == null) throw new Error('Form is not provided');
	const { getValues, watch } = form;
	if (typeof getValues !== 'function' || typeof watch !== 'function')
		throw new Error('Form is not provided');

	// Create a promise chain to make sure async callbacks are called in order
	const chain = useRef<Promise<true | void>>(Promise.resolve(true));

	// Disable lint warning because this hook is a wrapper for useCallback
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const onWatch = useCallback(callback, deps);

	useEffect(() => {
		chain.current = chain.current
			// If this is the first time, we don't need to wait for a form change
			.then((initCheck) => initCheck && onWatch(getValues(), {}))
			.finally(noop);

		return watch((_, info) => {
			chain.current = chain.current.then(() => onWatch(getValues(), info)).finally(noop);
		}).unsubscribe;
	}, [watch, onWatch, getValues]);
}
