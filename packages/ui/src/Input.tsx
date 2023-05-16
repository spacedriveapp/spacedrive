import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { Eye, EyeSlash, Icon, IconProps, MagnifyingGlass } from 'phosphor-react';
import { PropsWithChildren, createElement, forwardRef, isValidElement, useState } from 'react';
import { Button } from './Button';

export interface InputBaseProps extends VariantProps<typeof inputStyles> {
	icon?: Icon | React.ReactNode;
	iconPosition?: 'left' | 'right';
	right?: React.ReactNode;
}

export type InputProps = InputBaseProps & Omit<React.ComponentProps<'input'>, 'size'>;

export type TextareaProps = InputBaseProps & React.ComponentProps<'textarea'>;

export const inputSizes = {
	sm: 'h-[30px]',
	md: 'h-[34px]',
	lg: 'h-[38px]'
};

export const inputStyles = cva(
	[
		'rounded-md border text-sm leading-7',
		'shadow-sm outline-none transition-all focus-within:ring-2',
		'text-ink'
	],
	{
		variants: {
			variant: {
				default: [
					'border-app-line bg-app-input placeholder-ink-faint focus-within:bg-app-focus',
					'focus-within:border-app-divider/80 focus-within:ring-app-selected/30'
				]
			},
			error: {
				true: 'border-red-500 focus-within:border-red-500 focus-within:ring-red-400/30'
			},
			size: inputSizes
		},
		defaultVariants: {
			variant: 'default',
			size: 'sm'
		}
	}
);

export const Input = forwardRef<HTMLInputElement, InputProps>(
	({ variant, size, right, icon, iconPosition = 'left', className, error, ...props }, ref) => (
		<div
			className={clsx(
				'group flex',
				inputStyles({ variant, size: right && !size ? 'md' : size, error, className })
			)}
		>
			<div
				className={clsx(
					'flex h-full flex-1 overflow-hidden',
					iconPosition === 'right' && 'flex-row-reverse'
				)}
			>
				{icon && (
					<div
						className={clsx(
							'flex h-full items-center',
							iconPosition === 'left' ? 'pl-[10px] pr-2' : 'pl-2 pr-[10px]'
						)}
					>
						{isValidElement(icon)
							? icon
							: createElement<IconProps>(icon as Icon, {
									size: 18,
									className: 'text-gray-350'
							  })}
					</div>
				)}

				<input
					className={clsx(
						'flex-1 truncate border-none bg-transparent px-3 text-sm outline-none placeholder:text-ink-faint',
						(right || (icon && iconPosition === 'right')) && 'pr-0',
						icon && iconPosition === 'left' && 'pl-0'
					)}
					ref={ref}
					{...props}
				/>
			</div>

			{right && (
				<div
					className={clsx(
						'flex h-full min-w-[12px] items-center',
						size === 'lg' ? 'px-[5px]' : 'px-1'
					)}
				>
					{right}
				</div>
			)}
		</div>
	)
);

export const SearchInput = forwardRef<HTMLInputElement, InputProps>((props, ref) => (
	<Input {...props} ref={ref} icon={MagnifyingGlass} />
));

export const TextArea = ({ size, variant, error, ...props }: TextareaProps) => {
	return (
		<textarea
			{...props}
			className={clsx(
				'h-auto px-3 py-2',
				inputStyles({ size, variant, error }),
				props.className
			)}
		/>
	);
};

export function Label(props: PropsWithChildren<{ slug?: string }>) {
	return (
		<label className="text-sm font-bold" htmlFor={props.slug}>
			{props.children}
		</label>
	);
}

interface PasswordInputProps extends InputProps {
	buttonClassnames?: string;
}

export const PasswordInput = forwardRef<HTMLInputElement, PasswordInputProps>((props, ref) => {
	const [showPassword, setShowPassword] = useState(false);

	const CurrentEyeIcon = showPassword ? EyeSlash : Eye;

	return (
		<Input
			{...props}
			type={showPassword ? 'text' : 'password'}
			ref={ref}
			right={
				<Button
					tabIndex={0}
					onClick={() => setShowPassword(!showPassword)}
					size="icon"
					className={clsx(props.buttonClassnames)}
				>
					<CurrentEyeIcon className="!pointer-events-none h-4 w-4" />
				</Button>
			}
		/>
	);
});
