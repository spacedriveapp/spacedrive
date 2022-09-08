import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { Icon } from 'phosphor-react';
import React, { PropsWithChildren } from 'react';

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
}

const MENU_CLASSES = `
  flex flex-col
  min-w-[11rem]  m-2 space-y-1
  text-left text-sm dark:text-gray-100 text-gray-800
  bg-gray-50 border-gray-200 dark:bg-gray-600
  border border-gray-300 dark:border-gray-500
  shadow-2xl shadow-gray-300 dark:shadow-gray-950 
  select-none cursor-default rounded-lg 
`;

export const OverlayPanel = ({
	trigger,
	children,
	className,
	...props
}: PropsWithChildren<Props>) => {
	return (
		<DropdownMenu.Root>
			<DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
			<DropdownMenu.Portal>
				<DropdownMenu.Content {...props} className={clsx(MENU_CLASSES, className)}>
					{children}
				</DropdownMenu.Content>
			</DropdownMenu.Portal>
		</DropdownMenu.Root>
	);
};
