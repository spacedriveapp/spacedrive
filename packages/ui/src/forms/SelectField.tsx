import { FieldValues, useController, UseControllerProps } from 'react-hook-form';

import * as Root from '../Select';
import { FormField, useFormField, UseFormFieldProps } from './FormField';

export interface SelectFieldProps<T extends FieldValues>
	extends Omit<UseFormFieldProps, 'name'>,
		Omit<Root.SelectProps, 'value' | 'onChange'>,
		UseControllerProps<T> {}

export const SelectField = <T extends FieldValues>(props: SelectFieldProps<T>) => {
	const { formFieldProps, childProps } = useFormField(props);
	const { field } = useController({ name: props.name });

	return (
		<FormField {...formFieldProps}>
			<Root.Select
				{...childProps}
				className="w-full"
				value={field.value}
				onChange={field.onChange}
			/>
		</FormField>
	);
};
