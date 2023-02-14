import { forwardRef } from 'react';
import { useController } from 'react-hook-form';
import { z } from 'zod';
import * as RadioGroup from '../RadioGroup';
import { FormField, UseFormFieldProps, useFormField } from './FormField';

export interface RootProps extends UseFormFieldProps, RadioGroup.RootProps {
	name: string;
}

export const Root = forwardRef<HTMLDivElement, RootProps>((props, _) => {
	const {
		field: { onChange, ...field }
	} = useController(props);
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<RadioGroup.Root
				{...childProps}
				{...field}
				/* No-op so that only onValueChange is used */
				onChange={() => {}}
				onValueChange={onChange}
			/>
		</FormField>
	);
});

export { Item } from '../RadioGroup';

type Options = [z.ZodLiteral<string>, z.ZodLiteral<string>, ...z.ZodLiteral<string>[]];

export function options<T extends Options>(data: T) {
	const schema = z.union(data);

	return {
		schema,
		details: <Details extends object>(details: Record<z.infer<z.ZodUnion<T>>, Details>) => ({
			schema,
			options: Object.entries(details).map(([value, details]) => ({
				value,
				...(details as any)
			})) as {
				[Value in keyof T]: {
					value: Value;
				} & Details;
			}
		})
	};
}
