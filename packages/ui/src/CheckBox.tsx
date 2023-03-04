import { VariantProps, cva } from 'class-variance-authority';
import { ComponentProps, forwardRef } from 'react';

const styles = cva(
	[
		'form-check-input float-left mt-1 mr-2 h-4 w-4 appearance-none rounded-sm border border-gray-300 bg-white bg-contain bg-center bg-no-repeat align-top transition duration-200',
		'checked:border-blue-600 checked:bg-blue-600 focus:outline-none '
	],
	{ variants: {} }
);

export interface CheckBoxProps extends ComponentProps<'input'>, VariantProps<typeof styles> {}

export const CheckBox = forwardRef<HTMLInputElement, CheckBoxProps>(
	({ className, ...props }, ref) => (
		<input {...props} type="checkbox" ref={ref} className={styles({ className })} />
	)
);
