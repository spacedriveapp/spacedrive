import { forwardRef } from 'react';
import { useController } from 'react-hook-form';
import * as Root from '../Switch';
import { FormField, UseFormFieldProps, useFormField } from './FormField';

export interface SwitchProps extends UseFormFieldProps, Root.SwitchProps {
	name: string;
}

export const Switch = forwardRef<HTMLButtonElement, SwitchProps>((props, ref) => {
	const { field } = useController(props);
	const { formFieldProps, childProps } = useFormField(props);

	return (
		<FormField {...formFieldProps}>
			<Root.Switch
				{...childProps}
				checked={field.value}
				onCheckedChange={field.onChange}
				ref={ref}
			/>
		</FormField>
	);
});
