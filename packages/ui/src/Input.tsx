import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { Eye, EyeSlash } from 'phosphor-react';
import { PropsWithChildren, forwardRef, useState } from 'react';
import { Button } from './Button';

export interface InputBaseProps extends VariantProps<typeof styles> {}

export type InputProps = InputBaseProps & Omit<React.ComponentProps<'input'>, 'size'>;

export type TextareaProps = InputBaseProps & React.ComponentProps<'textarea'>;

const styles = cva(
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

interface PasswordShowHideInputProps extends InputProps {
	buttonClassnames?: string;
}

export const PasswordShowHideInput = forwardRef<HTMLInputElement, PasswordShowHideInputProps>(
	({ variant, size, className, ...props }, ref) => {
		const [showPassword, setShowPassword] = useState(false);
		const CurrentEyeIcon = showPassword ? EyeSlash : Eye;
		return (
			<span className="relative flex-grow">
				<Button
					onClick={() => setShowPassword(!showPassword)}
					size="icon"
					className={clsx(
						'absolute top-1.5 bottom-1.5 right-2 m-auto w-[25px] border-none',
						props.buttonClassnames
					)}
				>
					<CurrentEyeIcon className="h-4 w-4" />
				</Button>
				<input
					{...props}
					type={showPassword ? 'text' : 'password'}
					ref={ref}
					className={styles({ variant, size, className })}
				/>
			</span>
		);
	}
);
