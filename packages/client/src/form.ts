import { zodResolver } from '@hookform/resolvers/zod';
import { useCallback, useRef } from 'react';
import { useForm, UseFormProps } from 'react-hook-form';
import { z } from 'zod';

export interface UseZodFormProps<S extends z.ZodObject<any>>
	extends Exclude<UseFormProps<z.infer<S>>, 'resolver'> {
	schema?: S;
}

export function useZodForm<S extends z.ZodObject<any>>(props?: UseZodFormProps<S>) {
	const { schema, ...formProps } = props ?? {};

	return useForm<z.infer<S>>({
		...formProps,
		resolver: zodResolver(schema || z.object({}))
	});
}

export function useMultiZodForm<S extends Record<string, z.ZodObject<any>>>({
	schemas,
	defaultValues,
	onData
}: {
	schemas: S;
	defaultValues: {
		[K in keyof S]?: UseZodFormProps<S[K]>['defaultValues'];
	};
	onData?: (data: { [K in keyof S]?: z.infer<S[K]> }) => any;
}) {
	const formsData = useRef<{ [K in keyof S]?: z.infer<S[K]> }>({});

	return {
		useForm<K extends keyof S>(
			key: K,
			props?: Exclude<UseZodFormProps<S[K]>, 'schema' | 'defaultValues'>
		) {
			const form = useZodForm({
				...props,
				defaultValues: defaultValues[key],
				schema: schemas[key]
			});
			const handleSubmit = form.handleSubmit;

			form.handleSubmit = useCallback(
				(onValid, onError) =>
					handleSubmit((data, e) => {
						formsData.current[key] = data;
						onData?.(formsData.current);
						return onValid(data, e);
					}, onError),
				[handleSubmit, key]
			);

			return form;
		},
		handleSubmit:
			(
				onValid: (data: { [K in keyof S]: z.infer<S[K]> }) => any | Promise<any>,
				onError?: (key: keyof S) => void
			) =>
			() => {
				for (const key of Object.keys(schemas)) {
					if (formsData.current[key] === undefined) {
						onError?.(key);
						return;
					}
				}

				return onValid(formsData.current as any);
			}
	};
}
