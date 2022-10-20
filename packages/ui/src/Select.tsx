import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from '@heroicons/react/24/solid';
import * as SelectPrimitive from '@radix-ui/react-select';
import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';

interface SelectProps {
	value: string;
	size?: 'sm' | 'md' | 'lg';
	className?: string;
	onChange: (value: string) => void;
}

export function Select(props: PropsWithChildren<SelectProps>) {
	return (
		<SelectPrimitive.Root
			defaultValue={props.value}
			value={props.value}
			onValueChange={props.onChange}
		>
			<SelectPrimitive.Trigger
				className={clsx(
					'inline-flex items-center pl-2 py-0.5 bg-gray-500 border rounded-md shadow outline-none border-gray-450/30 shadow-gray-900/10',
					props.className
				)}
			>
				<span className="flex-grow text-xs text-left truncate">
					<SelectPrimitive.Value />
				</span>

				<SelectPrimitive.Icon>
					<ChevronDouble className="w-3 h-3 mr-0.5 text-gray-300" />
				</SelectPrimitive.Icon>
			</SelectPrimitive.Trigger>

			<SelectPrimitive.Portal className="relative">
				<SelectPrimitive.Content className="absolute z-50 w-full p-1 border border-gray-200 rounded-md shadow-2xl bg-gray-50 dark:bg-black/50 backdrop-blur dark:border-gray-500 shadow-gray-300 dark:shadow-gray-950">
					<SelectPrimitive.ScrollUpButton className="flex items-center justify-center text-gray-700 dark:text-gray-300">
						<ChevronDownIcon />
					</SelectPrimitive.ScrollUpButton>
					<SelectPrimitive.Viewport>{props.children}</SelectPrimitive.Viewport>
					<SelectPrimitive.ScrollDownButton className="flex items-center justify-center text-gray-700 dark:text-gray-300"></SelectPrimitive.ScrollDownButton>
				</SelectPrimitive.Content>
			</SelectPrimitive.Portal>
		</SelectPrimitive.Root>
	);
}

export function SelectOption(props: PropsWithChildren<{ value: string }>) {
	return (
		<SelectPrimitive.Item
			className={clsx(
				'relative flex items-center pl-6 px-1 py-0.5 dark:text-white pr-4 text-xs rounded font-sm cursor-pointer focus:bg-gray-100 dark:focus:bg-primary-500',
				'radix-disabled:opacity-50',
				'focus:outline-none select-none'
			)}
			value={props.value}
		>
			<SelectPrimitive.ItemText>{props.children}</SelectPrimitive.ItemText>
			<SelectPrimitive.ItemIndicator className="absolute inline-flex items-center left-1">
				<CheckIcon className="w-4 h-4" />
			</SelectPrimitive.ItemIndicator>
		</SelectPrimitive.Item>
	);
}
