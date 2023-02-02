import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import * as SelectPrimitive from '@radix-ui/react-select';
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
					'bg-app-box inline-flex items-center border py-0.5 pl-2',
					'border-app-line shadow-app-shade/10 rounded-md shadow outline-none',
					props.className
				)}
			>
				<span className="flex-grow truncate text-left text-xs">
					<SelectPrimitive.Value />
				</span>

				<SelectPrimitive.Icon>
					<ChevronDouble className="text-ink-dull mr-0.5 h-3 w-3" />
				</SelectPrimitive.Icon>
			</SelectPrimitive.Trigger>

			<SelectPrimitive.Portal className="relative">
				<SelectPrimitive.Content className="bg-app-box border-app-line shadow-app-shade/20 absolute z-50 w-full rounded-md border p-1 shadow-2xl ">
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
				'relative flex items-center px-1 py-0.5 pl-6 pr-4 text-xs',
				'font-sm text-ink cursor-pointer select-none rounded',
				'radix-disabled:opacity-50 hover:bg-accent hover:text-white focus:outline-none '
			)}
			value={props.value}
		>
			<SelectPrimitive.ItemText>{props.children}</SelectPrimitive.ItemText>
			<SelectPrimitive.ItemIndicator className="absolute left-1 inline-flex items-center">
				<Check className="h-4 w-4" />
			</SelectPrimitive.ItemIndicator>
		</SelectPrimitive.Item>
	);
}
