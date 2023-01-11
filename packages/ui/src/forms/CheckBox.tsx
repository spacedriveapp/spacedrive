import { forwardRef } from 'react';

import { CheckBox as Root } from '../CheckBox';
import { FormField, UseFormFieldProps, useFormField } from './FormField';

export interface CheckBoxProps extends UseFormFieldProps {}

export const CheckBox = forwardRef<HTMLInputElement, CheckBoxProps>((props, ref) => {
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root {...childProps} ref={ref} />
		</FormField>
	);
});
