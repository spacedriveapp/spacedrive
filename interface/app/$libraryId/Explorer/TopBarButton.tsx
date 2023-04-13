import { cva } from 'class-variance-authority';
import { Check } from 'phosphor-react';
import { forwardRef } from 'react';
import { Button } from '@sd/ui';

export interface TopBarButtonProps {
	children: React.ReactNode;
	rounding?: 'none' | 'left' | 'right' | 'both';
	active?: boolean;
	className?: string;
	onClick?: () => void;
	checkIcon?: React.ReactNode;
}

const topBarButtonStyle = cva(
	'text-ink hover:text-ink text-md relative hover:bg-app-selected radix-state-open:bg-app-selected mr-[1px] flex border-none !p-0.5 font-medium outline-none transition-colors duration-100',
	{
		variants: {
			active: {
				true: 'bg-app-selected',
				false: 'bg-transparent'
			},
			rounding: {
				none: 'rounded-none',
				left: 'rounded-l-md rounded-r-none',
				right: 'rounded-r-md rounded-l-none',
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
	({ active, rounding, className, ...props }, ref) => {
		return (
			<Button {...props} ref={ref} className={topBarButtonStyle({ active, rounding, className })}>
				{props.children}
				{props.checkIcon && active && (
					<Check className="absolute right-2 m-0.5 h-5 w-5 text-ink-dull" />
				)}
			</Button>
		);
	}
);
