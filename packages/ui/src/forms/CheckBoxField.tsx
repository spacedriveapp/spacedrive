import { forwardRef } from 'react';

import { CheckBox as Root } from '../CheckBox';
import { FormField, useFormField, UseFormFieldProps } from './FormField';

export interface CheckBoxFieldProps extends UseFormFieldProps {}

export const CheckBoxField = forwardRef<HTMLInputElement, CheckBoxFieldProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root {...childProps} ref={ref} />
		</FormField>
	);
});
