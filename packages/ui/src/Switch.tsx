'use client';

import * as SwitchPrimitive from '@radix-ui/react-switch';
import { cva, VariantProps } from 'class-variance-authority';
import { forwardRef } from 'react';

export interface SwitchProps
	extends VariantProps<typeof switchStyles>,
		SwitchPrimitive.SwitchProps {
	thumbClassName?: string;
}

const switchStyles = cva(
	[
		'relative inline-flex shrink-0 transition',
		'items-center rounded-full p-1',
		'bg-app-line focus:outline-none focus:ring-1 focus:ring-accent focus:ring-offset-2 focus:ring-offset-app-selected radix-state-checked:bg-accent'
	],
	{
		variants: {
			size: {
				sm: 'h-[20px] w-[34px]',
				md: 'h-[25px] w-[47px]',
				lg: 'h-[30px] w-[55px]'
			}
		},
		defaultVariants: {
			size: 'md'
		}
	}
);
const thumbStyles = cva(
	['inline-block size-4 transition', 'rounded-full bg-white', 'shadow-sm shadow-app-shade/40'],
	{
		variants: {
			size: {
				sm: 'size-[12px] radix-state-checked:translate-x-[14px]',
				md: 'size-[19px] radix-state-checked:translate-x-[20px]',
				lg: 'size-6 radix-state-checked:translate-x-[23px]'
			}
		},
		defaultVariants: {
			size: 'md'
		}
	}
);

export const Switch = forwardRef<HTMLButtonElement, SwitchProps>(
	({ size, className, thumbClassName, ...props }, ref) => (
		<SwitchPrimitive.Root {...props} ref={ref} className={switchStyles({ size, className })}>
			<SwitchPrimitive.Thumb className={thumbStyles({ size, className: thumbClassName })} />
		</SwitchPrimitive.Root>
	)
);
