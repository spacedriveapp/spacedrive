'use client';

import * as RadixDM from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import React, {
	ContextType,
	createContext,
	PropsWithChildren,
	ReactNode,
	Suspense,
	useCallback,
	useContext,
	useRef,
	useState
} from 'react';
import { Link } from 'react-router-dom';

import {
	contextMenuClassNames,
	ContextMenuDivItem,
	contextMenuItemClassNames,
	ContextMenuItemProps,
	contextMenuSeparatorClassNames
} from './ContextMenu';

interface DropdownMenuProps
	extends RadixDM.MenuContentProps,
		Pick<RadixDM.DropdownMenuProps, 'onOpenChange'> {
	trigger: React.ReactNode;
	triggerClassName?: string;
	alignToTrigger?: boolean;
}

const DropdownMenuContext = createContext<boolean | null>(null);

export const useDropdownMenuContext = <T extends boolean>({ suspense }: { suspense?: T } = {}) => {
	const ctx = useContext(DropdownMenuContext);

	if (suspense && ctx === null) throw new Error('DropdownMenuContext.Provider not found!');

	return ctx as T extends true
		? NonNullable<ContextType<typeof DropdownMenuContext>>
		: NonNullable<ContextType<typeof DropdownMenuContext>> | undefined;
};

const Root = (props: PropsWithChildren<DropdownMenuProps>) => {
	const {
		alignToTrigger,
		onOpenChange,
		trigger,
		triggerClassName,
		asChild = true,
		className,
		children,
		...contentProps
	} = props;

	const [width, setWidth] = useState<number>();

	const measureRef = useCallback(
		(ref: HTMLButtonElement | null) => {
			if (alignToTrigger && ref) setWidth(ref.getBoundingClientRect().width);
		},
		[alignToTrigger]
	);

	return (
		<RadixDM.Root onOpenChange={onOpenChange}>
			<RadixDM.Trigger ref={measureRef} className={triggerClassName} asChild={asChild}>
				{trigger}
			</RadixDM.Trigger>
			<RadixDM.Portal>
				<Suspense fallback={null}>
					<RadixDM.Content
						className={clsx(
							contextMenuClassNames,
							width && '!min-w-0 max-w-none',
							className
						)}
						align="start"
						style={{ width }}
						{...contentProps}
					>
						<DropdownMenuContext.Provider value={true}>
							{children}
						</DropdownMenuContext.Provider>
					</RadixDM.Content>
				</Suspense>
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
	iconProps,
	keybind,
	variant,
	className,
	...props
}: RadixDM.MenuSubContentProps & ContextMenuItemProps & { trigger?: ReactNode }) => {
	return (
		<RadixDM.Sub>
			<RadixDM.SubTrigger className={contextMenuItemClassNames}>
				{props.trigger || (
					<ContextMenuDivItem
						rightArrow
						{...{ label, icon, iconProps, keybind, variant }}
					/>
				)}
			</RadixDM.SubTrigger>
			<RadixDM.Portal>
				<Suspense fallback={null}>
					<RadixDM.SubContent
						className={clsx(contextMenuClassNames, className)}
						{...props}
					/>
				</Suspense>
			</RadixDM.Portal>
		</RadixDM.Sub>
	);
};

interface DropdownItemProps extends ContextMenuItemProps {
	to?: string;
	selected?: boolean;
}

const Item = ({
	icon,
	iconProps,
	label,
	children,
	keybind,
	variant,
	className,
	selected,
	to,
	...props
}: DropdownItemProps & RadixDM.MenuItemProps) => {
	const ref = useRef<HTMLDivElement>(null);

	const renderInner = (
		// to style this, pass in variant
		<ContextMenuDivItem
			className={clsx(selected && 'bg-accent text-white')}
			{...{ icon, iconProps, label, keybind, variant, children }}
		/>
	);

	return (
		<RadixDM.Item ref={ref} className={clsx(contextMenuItemClassNames, className)} {...props}>
			{to ? (
				<Link to={to} onClick={() => ref.current?.click()}>
					{renderInner}
				</Link>
			) : (
				renderInner
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
