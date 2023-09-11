import { forwardRef } from 'react';

import * as Root from '../Input';
import { FormField, useFormField, UseFormFieldProps } from './FormField';

export interface TextAreaFieldProps extends UseFormFieldProps, Root.TextareaProps {
	name: string;
}

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextAreaFieldProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.TextArea {...childProps} ref={ref} error={formFieldProps.error !== undefined} />
		</FormField>
	);
});
