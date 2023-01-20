import * as SwitchPrimitive from '@radix-ui/react-switch';
import { VariantProps, cva } from 'class-variance-authority';
import { forwardRef } from 'react';

export interface SwitchProps
	extends VariantProps<typeof switchStyles>,
		SwitchPrimitive.SwitchProps {}

const switchStyles = cva(
	[
		'transition relative flex-shrink-0 inline-flex',
		'items-center rounded-full p-1',
		'bg-app-line radix-state-checked:bg-accent'
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
			size: 'lg'
		}
	}
);
const thumbStyles = cva(
	[
		'transition inline-block w-4 h-4',
		'transform rounded-full bg-white',
		'shadow-sm shadow-app-shade/40'
	],
	{
		variants: {
			size: {
				sm: 'w-[12px] h-[12px] radix-state-checked:translate-x-[14px]',
				md: 'w-[19px] h-[19px] radix-state-checked:translate-x-[20px]',
				lg: 'w-6 h-6 radix-state-checked:translate-x-[23px]'
			}
		},
		defaultVariants: {
			size: 'lg'
		}
	}
);

export const Switch = forwardRef<HTMLButtonElement, SwitchProps>(
	({ size, className, ...props }, ref) => (
		<SwitchPrimitive.Root {...props} ref={ref} className={switchStyles({ size, className })}>
			<SwitchPrimitive.Thumb className={thumbStyles({ size, className })} />
		</SwitchPrimitive.Root>
	)
);
