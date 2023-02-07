import { useEffect } from 'react';
import { FieldValues, UseFormReturn } from 'react-hook-form';
import { useDebouncedCallback } from 'use-debounce';
import { useCurrentLibrary } from '@sd/client';

export function useDebouncedForm<TFieldValues extends FieldValues = FieldValues, TContext = any>(
	form: UseFormReturn<{ id: string } & object, TContext>,
	callback: (data: any) => void,
	args?: { disableResetOnLibraryChange?: boolean }
) {
	const { library } = useCurrentLibrary();
	const debounced = useDebouncedCallback(callback, 500);

	// listen for any form changes
	form.watch(debounced);

	// persist unchanged data when the component is unmounted
	useEffect(() => () => debounced.flush(), [debounced]);

	// ensure the form is updated when the library changes
	useEffect(() => {
		if (args?.disableResetOnLibraryChange !== true && library?.uuid !== form.getValues('id')) {
			form.reset({ id: library?.uuid, ...library?.config });
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [library, form.getValues, form.reset, args?.disableResetOnLibraryChange]);
}
