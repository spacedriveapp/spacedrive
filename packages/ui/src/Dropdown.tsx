import { Menu, Transition } from '@headlessui/react';
import { ChevronDownIcon } from '@heroicons/react/24/solid';
import clsx from 'clsx';
import { Fragment, PropsWithChildren } from 'react';
import { Link } from 'react-router-dom';

import { Button } from './Button';

export type DropdownItem = (
	| {
			name: string;
			icon?: any;
			selected?: boolean;
			to?: string;
			wrapItemComponent?: React.FC<PropsWithChildren>;
	  }
	| {
			name: string;
			icon?: any;
			disabled?: boolean;
			selected?: boolean;
			onPress?: () => any;
			to?: string;
			wrapItemComponent?: React.FC<PropsWithChildren>;
	  }
)[];

export interface DropdownProps {
	items: DropdownItem[];
	buttonText?: string;
	buttonTextClassName?: string;
	buttonProps?: React.ComponentProps<typeof Button>;
	buttonComponent?: React.ReactNode;
	buttonIcon?: any;
	className?: string;
	itemsClassName?: string;
	itemButtonClassName?: string;
	align?: 'left' | 'right';
}

export const Dropdown: React.FC<DropdownProps> = (props) => {
	return (
		<div className={clsx('w-full mt-2', props.className)}>
			<Menu as="div" className="relative flex w-full text-left">
				<Menu.Button as="div" className="flex-1 outline-none">
					{props.buttonComponent ? (
						props.buttonComponent
					) : (
						<Button size="sm" {...props.buttonProps}>
							{props.buttonIcon}
							{props.buttonText && (
								<>
									<span className={clsx('w-32 truncate', props.buttonTextClassName)}>
										{props.buttonText}
									</span>
									<div className="flex-grow" />
									<ChevronDownIcon
										className="w-5 h-5 ml-2 -mr-1 text-violet-200 hover:text-violet-100 "
										aria-hidden="true"
									/>
								</>
							)}
						</Button>
					)}
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
						{props.items.map((item, index) => (
							<div key={index} className="px-1 py-1 space-y-[2px]">
								{item.map((button, index) => (
									<Menu.Item key={index}>
										{({ active }) => {
											const WrappedItem: any = button.wrapItemComponent
												? button.wrapItemComponent
												: (props: React.PropsWithChildren) => <>{props.children}</>;

											return (
												<WrappedItem>
													{button.to ? (
														<Link
															to={button.to}
															className={clsx(
																'text-sm group flex grow shrink-0 rounded items-center w-full whitespace-nowrap px-2 py-1 mb-[2px] dark:hover:bg-gray-650 disabled:opacity-50 disabled:cursor-not-allowed',
																{
																	'bg-gray-300 dark:bg-primary dark:hover:bg-primary':
																		button.selected
																	// 'text-gray-900 dark:text-gray-200': !active
																},
																props.itemButtonClassName
															)}
														>
															{button.icon && (
																<button.icon
																	className={clsx('mr-2 w-4 h-4', {
																		'dark:text-gray-100': active,
																		'text-gray-600 dark:text-gray-200': !active
																	})}
																/>
															)}
															<span className="text-left">{button.name}</span>
														</Link>
													) : (
														<button
															onClick={(button as any).onPress}
															disabled={(button as any)?.disabled === true}
															className={clsx(
																'text-sm group flex grow shrink-0 rounded items-center w-full whitespace-nowrap px-2 py-1 mb-[2px] dark:hover:bg-gray-650 disabled:opacity-50 disabled:cursor-not-allowed',
																{
																	'bg-gray-300 dark:bg-primary dark:hover:bg-primary':
																		button.selected
																	// 'text-gray-900 dark:text-gray-200': !active
																},
																props.itemButtonClassName
															)}
														>
															{button.icon && (
																<button.icon
																	className={clsx('mr-2 w-4 h-4', {
																		'dark:text-gray-100': active,
																		'text-gray-600 dark:text-gray-200': !active
																	})}
																/>
															)}
															<span className="text-left">{button.name}</span>
														</button>
													)}
												</WrappedItem>
											);
										}}
									</Menu.Item>
								))}
							</div>
						))}
					</Menu.Items>
				</Transition>
			</Menu>
		</div>
	);
};
