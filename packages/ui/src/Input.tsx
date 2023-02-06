import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { PropsWithChildren, forwardRef } from 'react';

export interface InputBaseProps extends VariantProps<typeof styles> {}

export type InputProps = InputBaseProps & React.InputHTMLAttributes<HTMLInputElement>;

export type TextareaProps = InputBaseProps & React.TextareaHTMLAttributes<HTMLTextAreaElement>;

const styles = cva(
	[
		'rounded-md border px-3 py-1 text-sm leading-7',
		'shadow-sm outline-none transition-all focus:ring-2'
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
	({ variant, size, className, ...props }, ref) => (
		<input {...props} ref={ref} className={styles({ variant, size, className })} />
	)
);

export const TextArea = ({ size, variant, ...props }: TextareaProps) => {
	return <textarea {...props} className={clsx(styles({ size, variant }), props.className)} />;
};

export function Label(props: PropsWithChildren<{ slug?: string }>) {
	return (
		<label className="text-sm font-bold" htmlFor={props.slug}>
			{props.children}
		</label>
	);
}
