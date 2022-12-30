import { useEffect } from 'react';
import { FieldValues, UseFormReturn } from 'react-hook-form';
import { useDebouncedCallback } from 'use-debounce';

// Same as useDebouncedForm, just a bit more general to use it with all forms.
export function useAutoForm<TFieldValues extends FieldValues = FieldValues, TContext = any>(
	form: UseFormReturn<TFieldValues, TContext>,
	callback: (data: any) => void,
	/**
	 *Wait time in miliseconds
	 */
	waitTime = 500
) {
	const debounced = useDebouncedCallback(callback, waitTime);

	// listen for any form changes
	form.watch(debounced);

	// persist unchanged data when the component is unmounted
	useEffect(() => () => debounced.flush(), [debounced]);
}
