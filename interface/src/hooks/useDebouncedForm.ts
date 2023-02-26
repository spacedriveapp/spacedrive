import { useEffect } from 'react';
import { FieldValues, UseFormReturn, WatchObserver } from 'react-hook-form';
import { useDebouncedCallback } from 'use-debounce';

export function useDebouncedFormWatch<
	TFieldValues extends FieldValues = FieldValues,
	TContext = any
>(form: UseFormReturn<TFieldValues, TContext>, callback: WatchObserver<TFieldValues>) {
	const debounced = useDebouncedCallback(callback, 500);

	// listen for any form changes
	useEffect(() => {
		const { unsubscribe } = form.watch(debounced);
		return () => unsubscribe();
	}, [form, debounced]);

	// persist unchanged data when the component is unmounted
	useEffect(() => () => debounced.flush(), [debounced, form]);
}
