import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import * as RS from '@radix-ui/react-select';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { Check } from 'phosphor-react';
import { PropsWithChildren } from 'react';

export const selectStyles = cva(
	[
		'rounded-md border text-sm flex pl-3 pr-[10px] items-center justify-between',
		'shadow-sm outline-none transition-all focus:ring-2',
		'radix-placeholder:text-ink-faint'
	],
	{
		variants: {
			variant: {
				default: [
					'bg-app-input focus:bg-app-focus',
					'border-app-line focus:border-app-divider/80',
					'focus:ring-app-selected/30'
				]
			},

			size: {
				sm: 'h-[30px]',
				md: 'h-[34px]',
				lg: 'h-[38px]'
			}
		},
		defaultVariants: {
			variant: 'default',
			size: 'sm'
		}
	}
);

export interface SelectProps
	extends VariantProps<typeof selectStyles>,
		Omit<RS.SelectTriggerProps, 'value' | 'onChange'> {
	value: string;
	onChange: (value: string) => void;
	placeholder?: string;
	className?: string;
	disabled?: boolean;
}

export function Select({
	value,
	onChange,
	placeholder,
	className,
	disabled,
	size,
	children,
	...props
}: PropsWithChildren<SelectProps>) {
	return (
		<RS.Root defaultValue={value} value={value} onValueChange={onChange} disabled={disabled}>
			<RS.Trigger className={selectStyles({ size: size, className })} {...props}>
				<RS.Value placeholder={placeholder} />
				<RS.Icon className="ml-2">
					<ChevronDouble className="text-ink-dull" />
				</RS.Icon>
			</RS.Trigger>

			<RS.Portal>
				<RS.Content className="z-50 rounded-md border border-app-line bg-app-box shadow-2xl shadow-app-shade/20 ">
					<RS.Viewport className="p-1">{children}</RS.Viewport>
				</RS.Content>
			</RS.Portal>
		</RS.Root>
	);
}

export function SelectOption(props: PropsWithChildren<{ value: string; default?: boolean }>) {
	return (
		<RS.Item
			value={props.value}
			defaultChecked={props.default}
			className={clsx(
				'relative flex h-6 cursor-pointer select-none items-center rounded pl-6 pr-3',
				'text-sm text-ink radix-highlighted:text-white',
				'focus:outline-none radix-disabled:opacity-50 radix-highlighted:bg-accent '
			)}
		>
			<RS.ItemText>{props.children}</RS.ItemText>
			<RS.ItemIndicator className="absolute left-1 inline-flex items-center">
				<Check className="h-4 w-4" />
			</RS.ItemIndicator>
		</RS.Item>
	);
}
