import { Check } from '@phosphor-icons/react';
import { cva } from 'class-variance-authority';
import { forwardRef } from 'react';
import { Button } from '@sd/ui';

export interface TopBarButtonProps {
	children: React.ReactNode;
	rounding?: 'none' | 'left' | 'right' | 'both';
	active?: boolean;
	className?: string;
	onClick?: () => void;
	checkIcon?: React.ReactNode;
	disabled?: boolean;
}

const topBarButtonStyle = cva(
	'text-md relative mr-px flex border-none !p-0.5 font-medium text-ink outline-none transition-colors duration-100 hover:bg-app-selected hover:text-ink radix-state-open:bg-app-selected',
	{
		variants: {
			active: {
				true: '!bg-app-selected',
				false: 'bg-transparent'
			},
			rounding: {
				none: 'rounded-none',
				left: 'rounded-l-md rounded-r-none',
				right: 'rounded-l-none rounded-r-md',
				both: 'rounded-md'
			}
		},
		defaultVariants: {
			active: false,
			rounding: 'both'
		}
	}
);

export default forwardRef<HTMLButtonElement, TopBarButtonProps>(
	({ active, rounding, className, checkIcon, ...props }, ref) => {
		return (
			<Button
				{...props}
				ref={ref}
				className={topBarButtonStyle({ active, rounding, className })}
			>
				{props.children}
				{checkIcon && active && (
					<Check className="absolute right-2 m-0.5 size-5 text-ink-dull" />
				)}
			</Button>
		);
	}
);
