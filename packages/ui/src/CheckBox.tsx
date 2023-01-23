import { VariantProps, cva } from 'class-variance-authority';
import { ComponentProps, forwardRef } from 'react';

const styles = cva(
	[
		'form-check-input appearance-none h-4 w-4 border border-gray-300 rounded-sm bg-white transition duration-200 mt-1 align-top bg-no-repeat bg-center bg-contain float-left mr-2',
		'checked:bg-blue-600 checked:border-blue-600 focus:outline-none '
	],
	{ variants: {} }
);

export interface CheckBoxProps extends ComponentProps<'input'>, VariantProps<typeof styles> {}

export const CheckBox = forwardRef<HTMLInputElement, CheckBoxProps>(
	({ className, ...props }, ref) => (
		<input {...props} type="checkbox" ref={ref} className={styles({ className })} />
	)
);
