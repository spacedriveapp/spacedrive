import { Menu, Transition } from '@headlessui/react';
import { ChevronDownIcon } from '@heroicons/react/24/solid';
import { VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { Fragment, PropsWithChildren } from 'react';
import { Link } from 'react-router-dom';

import * as UI from '.';
import { cva, tw } from '.';

export const Section = tw.div`px-1 py-1 space-y-[2px]`;

const itemStyles = cva(
	'text-sm group flex grow shrink-0 rounded items-center w-full whitespace-nowrap px-2 py-1 mb-[2px] dark:hover:bg-gray-650 disabled:opacity-50 disabled:cursor-not-allowed',
	{
		variants: {
			selected: {
				true: 'bg-gray-300 dark:bg-primary dark:hover:bg-primary'
			},
			active: {
				true: ''
				//   false: 'text-gray-900 dark:text-gray-200'
			}
		}
	}
);

const itemIconStyles = cva('mr-2 w-4 h-4', {
	variants: {
		active: {
			true: 'dark:text-gray-100',
			false: 'text-gray-600 dark:text-gray-200'
		}
	}
});

type DropdownItemProps =
	| PropsWithChildren<{
			to?: string;
			className?: string;
			icon?: any;
			onClick?: () => void;
	  }> &
			VariantProps<typeof itemStyles>;

export const Item = ({ to, className, icon: Icon, children, ...props }: DropdownItemProps) => {
	let content = (
		<>
			{Icon && <Icon className={itemIconStyles(props)} />}
			<span className="text-left">{children}</span>
		</>
	);

	return to ? (
		<Link {...props} to={to} className={clsx(itemStyles(props), className)}>
			{content}
		</Link>
	) : (
		<button {...props} className={clsx(itemStyles(props), className)}>
			{content}
		</button>
	);
};

export const Button = ({ children, ...props }: UI.ButtonProps) => {
	return (
		<UI.Button size="sm" {...props}>
			{children}
			<div className="flex-grow" />
			<ChevronDownIcon
				className="w-5 h-5 ml-2 -mr-1 text-violet-200 hover:text-violet-100"
				aria-hidden="true"
			/>
		</UI.Button>
	);
};

export interface DropdownRootProps {
	button: React.ReactNode;
	className?: string;
	itemsClassName?: string;
	align?: 'left' | 'right';
}

export const Root = (props: PropsWithChildren<DropdownRootProps>) => {
	return (
		<Menu as="div" className={clsx('relative flex w-full text-left', props.className)}>
			<Menu.Button as="div" className="flex-1 outline-none">
				{props.button}
			</Menu.Button>

			<Transition
				as={Fragment}
				enter="transition duration-100 ease-out"
				enterFrom="transform scale-95 opacity-0"
				enterTo="transform scale-100 opacity-100"
				leave="transition duration-75 ease-out"
				leaveFrom="transform scale-100 opacity-100"
				leaveTo="transform scale-95 opacity-0"
			>
				<Menu.Items
					className={clsx(
						'absolute z-50 min-w-fit w-full bg-white border divide-y divide-gray-100 rounded shadow-xl top-full dark:bg-gray-550 dark:divide-gray-500 dark:border-gray-600 ring-1 ring-black ring-opacity-5 focus:outline-none',
						props.itemsClassName,
						{ 'left-0': props.align === 'left' },
						{ 'right-0': props.align === 'right' }
					)}
				>
					{props.children}
				</Menu.Items>
			</Transition>
		</Menu>
	);
};
