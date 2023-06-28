import * as Checkbox from '@radix-ui/react-checkbox';
import { VariantProps, cva } from 'class-variance-authority';
import { Check } from 'phosphor-react';
import { ComponentProps, forwardRef } from 'react';

const styles = cva(
	[
		'form-check-input float-left mr-2 mt-1 h-4 w-4 appearance-none rounded-sm border border-gray-300 bg-white bg-contain bg-center bg-no-repeat align-top transition duration-200',
		'checked:border-accent checked:bg-accent checked:hover:bg-accent/80 focus:outline-none'
	],
	{ variants: {} }
);

export interface CheckBoxProps extends ComponentProps<'input'>, VariantProps<typeof styles> {}

export const CheckBox = forwardRef<HTMLInputElement, CheckBoxProps>(
	({ className, ...props }, ref) => (
		<input {...props} type="checkbox" ref={ref} className={styles({ className })} />
	)
);

export interface RadixCheckboxProps extends ComponentProps<typeof Checkbox.Root> {
	label?: string;
}

// TODO: Replace above with this, requires refactor of usage
export const RadixCheckbox = (props: RadixCheckboxProps) => (
	<div className="flex items-center">
		<Checkbox.Root
			className="flex h-[17px] w-[17px] shrink-0 items-center justify-center rounded-md border border-app-line bg-app-button radix-state-checked:bg-accent"
			id={props.name}
			{...props}
		>
			<Checkbox.Indicator className="text-white">
				<Check weight="bold" size={14} />
			</Checkbox.Indicator>
		</Checkbox.Root>
		{props.label && (
			<label className="ml-2 text-sm font-medium" htmlFor={props.name}>
				{props.label}
			</label>
		)}
	</div>
);
