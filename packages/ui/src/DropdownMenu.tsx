import * as RadixDM from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import React, { PropsWithChildren, Suspense, useContext, useRef } from 'react';
import { Link } from 'react-router-dom';
import {
	ContextMenuItemProps,
	ItemInternals,
	contextMenuClasses,
	contextMenuItemStyles,
	contextMenuSeparatorClassNames,
	contextSubMenuTriggerClassNames
} from './ContextMenu';

interface Props extends RadixDM.MenuContentProps, Pick<RadixDM.DropdownMenuProps, 'onOpenChange'> {
	trigger: React.ReactNode;
	triggerClassName?: string;
	alignToParent?: boolean;
}

const Root = ({
	trigger,
	children,
	className,
	asChild = true,
	triggerClassName,
	alignToParent,
	onOpenChange,
	...props
}: PropsWithChildren<Props>) => {
	const triggerRef = useRef<HTMLButtonElement>(null);

	return (
		<RadixDM.Root modal={false} onOpenChange={onOpenChange}>
			<RadixDM.Trigger ref={triggerRef} asChild={asChild} className={triggerClassName}>
				{trigger}
			</RadixDM.Trigger>
			<RadixDM.Portal>
				<div>
					<div className="fixed inset-0"></div>
					<RadixDM.Content
						className={clsx(contextMenuClasses, 'w-44', className)}
						align="start"
						collisionPadding={5}
						style={{
							width: alignToParent ? triggerRef.current?.offsetWidth : undefined
						}}
						{...props}
					>
						{children}
					</RadixDM.Content>
				</div>
			</RadixDM.Portal>
		</RadixDM.Root>
	);
};

const Separator = () => <RadixDM.Separator className={contextMenuSeparatorClassNames} />;

const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixDM.MenuSubContentProps & ContextMenuItemProps) => {
	return (
		<RadixDM.Sub>
			<RadixDM.SubTrigger className={contextSubMenuTriggerClassNames}>
				<div
					className={contextMenuItemStyles({
						class: 'group-radix-state-open:bg-trinary/50 group-radix-state-open:text-primary'
					})}
				>
					<ItemInternals rightArrow {...{ label, icon }} />
				</div>
			</RadixDM.SubTrigger>
			<RadixDM.Portal>
				<Suspense fallback={null}>
					<RadixDM.SubContent
						className={clsx(contextMenuClasses, className)}
						collisionPadding={5}
						{...props}
					/>
				</Suspense>
			</RadixDM.Portal>
		</RadixDM.Sub>
	);
};

interface DropdownItemProps extends ContextMenuItemProps, RadixDM.MenuItemProps {
	to?: string;
}

const Item = ({
	icon,
	label,
	rightArrow,
	children,
	keybind,
	variant,
	className,
	to,
	...props
}: DropdownItemProps) => {
	return (
		<RadixDM.Item
			className={clsx(
				'text-menu-ink group cursor-default select-none py-0.5 focus:outline-none active:opacity-80',
				className
			)}
			{...props}
		>
			{to ? (
				<Link to={to} className={contextMenuItemStyles({ variant })}>
					{children ? children : <ItemInternals {...{ icon, label, rightArrow, keybind }} />}
				</Link>
			) : (
				<div className={contextMenuItemStyles({ variant })}>
					{children ? children : <ItemInternals {...{ icon, label, rightArrow, keybind }} />}
				</div>
			)}
		</RadixDM.Item>
	);
};

export const DropdownMenu = {
	Root,
	Item,
	Separator,
	SubMenu
};
