import * as RadixDM from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import React, { PropsWithChildren, Suspense, useCallback, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import {
	ContextMenuItemProps,
	ItemInternals,
	contextMenuClassNames,
	contextMenuItemClassNames,
	contextMenuItemStyles,
	contextMenuSeparatorClassNames
} from './ContextMenu';

interface DropdownMenuProps
	extends RadixDM.MenuContentProps,
		Pick<RadixDM.DropdownMenuProps, 'onOpenChange'> {
	trigger: React.ReactNode;
	triggerClassName?: string;
	alignToTrigger?: boolean;
	animate?: boolean;
}

const Root = ({
	trigger,
	children,
	className,
	asChild = true,
	triggerClassName,
	alignToTrigger,
	onOpenChange,
	animate,
	...props
}: PropsWithChildren<DropdownMenuProps>) => {
	const [width, setWidth] = useState<number>();

	const measureRef = useCallback((ref: HTMLButtonElement | null) => {
		alignToTrigger && ref && setWidth(ref.getBoundingClientRect().width);
	}, []);

	return (
		<RadixDM.Root modal={false} onOpenChange={onOpenChange}>
			<RadixDM.Trigger ref={measureRef} asChild={asChild} className={triggerClassName}>
				{trigger}
			</RadixDM.Trigger>
			<RadixDM.Portal>
				<div>
					<div className="fixed inset-0"></div>
					<RadixDM.Content
						className={clsx(
							contextMenuClassNames,
							animate && 'animate-in fade-in data-[side=bottom]:slide-in-from-top-2',
							'w-44',
							width && 'min-w-0',
							className
						)}
						align="start"
						collisionPadding={5}
						style={{ width }}
						{...props}
					>
						{children}
					</RadixDM.Content>
				</div>
			</RadixDM.Portal>
		</RadixDM.Root>
	);
};

const Separator = (props: { className?: string }) => (
	<RadixDM.Separator className={clsx(contextMenuSeparatorClassNames, props.className)} />
);

const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixDM.MenuSubContentProps & ContextMenuItemProps) => {
	return (
		<RadixDM.Sub>
			<RadixDM.SubTrigger className={contextMenuItemClassNames}>
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
						className={clsx(contextMenuClassNames, className)}
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
	selected?: boolean;
}

const Item = ({
	icon,
	iconProps,
	label,
	rightArrow,
	children,
	keybind,
	variant,
	className,
	selected,
	to,
	...props
}: DropdownItemProps) => {
	const ref = useRef<HTMLDivElement>(null);

	return (
		<RadixDM.Item
			className={clsx(
				'text-menu-ink group cursor-default select-none py-0.5 focus:outline-none active:opacity-80',
				className
			)}
			ref={ref}
			{...props}
		>
			{to ? (
				<Link
					to={to}
					className={contextMenuItemStyles({
						variant,
						className: clsx(selected && 'bg-accent')
					})}
					onClick={() => ref.current?.click()}
				>
					{children ? (
						<span className="truncate">{children}</span>
					) : (
						<ItemInternals {...{ icon, iconProps, label, rightArrow, keybind }} />
					)}
				</Link>
			) : (
				<div
					className={contextMenuItemStyles({ variant, className: clsx(selected && 'bg-accent') })}
				>
					{children || <ItemInternals {...{ icon, iconProps, label, rightArrow, keybind }} />}
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
