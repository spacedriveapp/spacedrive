'use client';

import { Menu, Transition } from '@headlessui/react';
import { ReactComponent as CaretDown } from '@sd/assets/svgs/caret.svg';
import { cva, VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef, Fragment, PropsWithChildren } from 'react';
import { Link } from 'react-router-dom';

import * as UI from '.';
import { tw } from './utils';

export const Section = tw.div`px-1 py-1 space-y-0.5`;

const itemStyles = cva(
	'group flex w-full shrink-0 grow items-center whitespace-nowrap rounded px-2 py-1 text-sm font-medium disabled:opacity-50',
	{
		variants: {
			selected: {
				true: 'bg-accent text-white hover:!bg-accent',
				undefined: 'hover:bg-sidebar-selected/40',
				false: 'hover:bg-sidebar-selected/40'
			},
			active: {
				true: 'bg-sidebar-selected/40 text-sidebar-ink'
			}
		}
	}
);

const itemIconStyles = cva('mr-2 size-4', {
	variants: {}
});

type DropdownItemProps = PropsWithChildren<{
	to?: string;
	className?: string;
	icon?: any;
	iconClassName?: string;
	onClick?: () => void;
}> &
	VariantProps<typeof itemStyles>;

export const Item = ({ to, className, icon: Icon, children, ...props }: DropdownItemProps) => {
	const content = (
		<>
			{Icon && (
				<Icon weight="bold" className={clsx(itemIconStyles(props), props.iconClassName)} />
			)}
			<span className="text-left">{children}</span>
		</>
	);
	return (
		<Menu.Item>
			{to ? (
				<Link {...props} to={to} className={clsx(itemStyles(props), className)}>
					{content}
				</Link>
			) : (
				<button {...props} className={clsx(itemStyles(props), className)}>
					{content}
				</button>
			)}
		</Menu.Item>
	);
};

export const Button = forwardRef<HTMLButtonElement, UI.ButtonProps>(
	({ children, className, ...props }, ref) => {
		return (
			<UI.Button
				size="sm"
				ref={ref}
				className={clsx('group flex text-left', className)}
				{...props}
			>
				{children}
				<span className="grow" />
				<CaretDown
					className="ml-2 w-[12px] shrink-0 translate-y-px text-ink-dull transition-transform ui-open:-translate-y-px ui-open:rotate-180 group-radix-state-open:-translate-y-px group-radix-state-open:rotate-180"
					aria-hidden="true"
				/>
			</UI.Button>
		);
	}
);

export interface DropdownRootProps {
	button: React.ReactNode;
	className?: string;
	itemsClassName?: string;
	align?: 'left' | 'right';
}

export const Root = (props: PropsWithChildren<DropdownRootProps>) => {
	return (
		<div className={props.className}>
			<Menu as="div" className={clsx('relative flex w-full justify-end text-left')}>
				<Menu.Button role="button" as="div" className="outline-none">
					{props.button}
				</Menu.Button>
				<Transition
					as={Fragment}
					enter="transition duration-100 ease-out"
					enterFrom="transform -translate-y-2 opacity-0"
					enterTo="transform translate-y-0 opacity-100"
					leave="transition duration-75 ease-out"
					leaveFrom="transform translate-y-0 opacity-100"
					leaveTo="transform -translate-y-2 opacity-0"
				>
					<Menu.Items
						className={clsx(
							'absolute top-full z-50 w-full min-w-fit space-y-0.5 divide-y divide-menu-line rounded-md border border-menu-line bg-menu text-menu-ink shadow-xl shadow-menu-shade/30 focus:outline-none',
							props.itemsClassName,
							{ 'left-0': props.align === 'left' },
							{ 'right-0': props.align === 'right' }
						)}
					>
						{props.children}
					</Menu.Items>
				</Transition>
			</Menu>
		</div>
	);
};
