import { FieldValues, UseControllerProps, useController } from 'react-hook-form';
import * as Root from '../Select';
import { FormField, UseFormFieldProps, useFormField } from './FormField';

export interface SelectProps<T extends FieldValues>
	extends Omit<UseFormFieldProps, 'name'>,
		Omit<Root.SelectProps, 'value' | 'onChange'>,
		UseControllerProps<T> {}

export const Select = <T extends FieldValues>(props: SelectProps<T>) => {
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
