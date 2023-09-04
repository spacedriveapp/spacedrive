import { forwardRef } from 'react';
import * as Root from '../Input';
import { useFormField } from './FormField';
import { FormField, UseFormFieldProps } from './FormField';

export interface TextareaProps extends UseFormFieldProps, Root.TextareaProps {
	name: string;
}

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextareaProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.TextArea {...childProps} ref={ref} error={formFieldProps.error !== undefined} />
		</FormField>
	);
});
