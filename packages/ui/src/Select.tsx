import * as SelectPrimitive from '@radix-ui/react-select';
import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import clsx from 'clsx';
import { CaretDown, Check } from 'phosphor-react';
import { PropsWithChildren } from 'react';

interface SelectProps {
	value: string;
	size?: 'sm' | 'md' | 'lg';
	className?: string;
	onChange: (value: string) => void;
	disabled?: boolean;
}

export function Select(props: PropsWithChildren<SelectProps>) {
	return (
		<SelectPrimitive.Root
			defaultValue={props.value}
			value={props.value}
			onValueChange={props.onChange}
			disabled={props.disabled}
		>
			<SelectPrimitive.Trigger
				className={clsx(
					'inline-flex items-center pl-2 py-0.5 bg-app-box border',
					'rounded-md shadow outline-none border-app-line shadow-app-shade/10',
					props.className
				)}
			>
				<span className="flex-grow text-xs text-left truncate">
					<SelectPrimitive.Value />
				</span>

				<SelectPrimitive.Icon>
					<ChevronDouble className="w-3 h-3 mr-0.5 text-ink-dull" />
				</SelectPrimitive.Icon>
			</SelectPrimitive.Trigger>

			<SelectPrimitive.Portal className="relative">
				<SelectPrimitive.Content className="absolute z-50 w-full p-1 border rounded-md shadow-2xl bg-app-box border-app-line shadow-app-shade/20 ">
					<SelectPrimitive.ScrollUpButton className="hidden ">
						<CaretDown />
					</SelectPrimitive.ScrollUpButton>
					<SelectPrimitive.Viewport>{props.children}</SelectPrimitive.Viewport>
					<SelectPrimitive.ScrollDownButton className="hidden "></SelectPrimitive.ScrollDownButton>
				</SelectPrimitive.Content>
			</SelectPrimitive.Portal>
		</SelectPrimitive.Root>
	);
}

export function SelectOption(props: PropsWithChildren<{ value: string }>) {
	return (
		<SelectPrimitive.Item
			className={clsx(
				'relative flex items-center pl-6 px-1 py-0.5 pr-4 text-xs',
				'rounded font-sm cursor-pointer select-none text-ink',
				'focus:outline-none hover:text-white radix-disabled:opacity-50 hover:bg-accent '
			)}
			value={props.value}
		>
			<SelectPrimitive.ItemText>{props.children}</SelectPrimitive.ItemText>
			<SelectPrimitive.ItemIndicator className="absolute inline-flex items-center left-1">
				<Check className="w-4 h-4" />
			</SelectPrimitive.ItemIndicator>
		</SelectPrimitive.Item>
	);
}
