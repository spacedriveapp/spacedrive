import { forwardRef } from 'react';
import * as Root from '../Input';
import { FormField, UseFormFieldProps, useFormField } from './FormField';

export interface InputProps extends UseFormFieldProps, Root.InputProps {
	name: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.Input {...childProps} ref={ref} />
		</FormField>
	);
});

export const PasswordShowHideInput = forwardRef<HTMLInputElement, InputProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.PasswordShowHideInput {...childProps} ref={ref} />
		</FormField>
	);
});
