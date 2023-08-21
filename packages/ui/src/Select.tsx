import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import * as RS from '@radix-ui/react-select';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { Check } from 'phosphor-react';
import { PropsWithChildren, forwardRef } from 'react';

export const selectStyles = cva(
	[
		'flex items-center justify-between rounded-md border py-0.5 pl-3 pr-[10px] text-sm',
		'shadow-sm outline-none transition-all focus:ring-2',
		'text-ink radix-placeholder:text-ink-faint'
	],
	{
		variants: {
			variant: {
				default: ['bg-app-input', 'border-app-line']
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

export interface SelectProps<TValue extends string = string>
	extends VariantProps<typeof selectStyles> {
	value: TValue;
	onChange: (value: TValue) => void;
	placeholder?: string;
	className?: string;
	disabled?: boolean;
}

export const Select = forwardRef(
	<TValue extends string = string>(
		props: PropsWithChildren<SelectProps<TValue>>,
		ref: React.ForwardedRef<HTMLDivElement>
	) => (
		<div ref={ref}>
			<RS.Root
				defaultValue={props.value}
				value={props.value}
				onValueChange={props.onChange}
				disabled={props.disabled}
			>
				<RS.Trigger
					className={selectStyles({ size: props.size, className: props.className })}
				>
					<RS.Value placeholder={props.placeholder} />
					<RS.Icon className="ml-2">
						<ChevronDouble className="text-ink-dull" />
					</RS.Icon>
				</RS.Trigger>

				<RS.Portal>
					<RS.Content className="z-50 rounded-md border border-app-line bg-app-box shadow-2xl shadow-app-shade/20 ">
						<RS.Viewport className="p-1">{props.children}</RS.Viewport>
					</RS.Content>
				</RS.Portal>
			</RS.Root>
		</div>
	)
) as <TValue extends string = string>(
	props: PropsWithChildren<SelectProps<TValue>> & { ref?: React.ForwardedRef<HTMLDivElement> }
) => JSX.Element;

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
