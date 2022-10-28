import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { PropsWithChildren, forwardRef } from 'react';

export interface InputBaseProps extends VariantProps<typeof inputStyles> {}

export type InputProps = InputBaseProps & React.InputHTMLAttributes<HTMLInputElement>;

export type TextareaProps = InputBaseProps & React.TextareaHTMLAttributes<HTMLTextAreaElement>;

const inputStyles = cva(
	[
		'px-3 py-1 text-sm rounded-md border leading-7',
		'outline-none shadow-sm focus:ring-2 transition-all'
	],
	{
		variants: {
			variant: {
				default: [
					'bg-app-input focus:bg-app-focus placeholder-ink-faint border-app-line',
					'focus:ring-app-selected/30 focus:border-app-divider/80'
				]
			},
			size: {
				sm: 'text-sm',
				md: 'text-base'
			}
		},
		defaultVariants: {
			variant: 'default'
		}
	}
);

export const Input = forwardRef<HTMLInputElement, InputProps>(
	({ size, variant, ...props }, ref) => {
		return (
			<input
				ref={ref}
				{...props}
				className={clsx(inputStyles({ size, variant }), props.className)}
			/>
		);
	}
);

export const TextArea = ({ size, variant, ...props }: TextareaProps) => {
	return <textarea {...props} className={clsx(inputStyles({ size, variant }), props.className)} />;
};

export function Label(props: PropsWithChildren<{ slug?: string }>) {
	return (
		<label className="text-sm font-bold" htmlFor={props.slug}>
			{props.children}
		</label>
	);
}
