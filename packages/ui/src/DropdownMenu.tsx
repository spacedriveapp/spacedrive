import * as RadixDM from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import React, {
	PropsWithChildren,
	Suspense,
	useCallback,
	useContext,
	useRef,
	useState
} from 'react';
import { Link } from 'react-router-dom';
import {
	ContextMenuDivItem,
	ContextMenuItemProps,
	contextMenuClassNames,
	contextMenuItemClassNames,
	contextMenuSeparatorClassNames
} from './ContextMenu';

interface DropdownMenuProps extends RadixDM.MenuContentProps {
	trigger: React.ReactNode;
	triggerClassName?: string;
	alignToTrigger?: boolean;
}

const context = React.createContext<boolean>(false);
export const useDropdownMenu = () => useContext(context);

const Root = ({
	trigger,
	children,
	className,
	asChild = true,
	triggerClassName,
	alignToTrigger,
	...props
}: PropsWithChildren<DropdownMenuProps>) => {
	const [width, setWidth] = useState<number>();

	const measureRef = useCallback(
		(ref: HTMLButtonElement | null) => {
			alignToTrigger && ref && setWidth(ref.getBoundingClientRect().width);
		},
		[alignToTrigger]
	);

	return (
		<RadixDM.Root>
			<RadixDM.Trigger ref={measureRef} asChild={asChild} className={triggerClassName}>
				{trigger}
			</RadixDM.Trigger>
			<RadixDM.Portal>
				<RadixDM.Content
					className={clsx(contextMenuClassNames, width && '!min-w-0', className)}
					align="start"
					style={{ width }}
					{...props}
				>
					<context.Provider value={true}>{children}</context.Provider>
				</RadixDM.Content>
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
				<ContextMenuDivItem rightArrow {...{ label, icon }} />
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
	rightArrow,
	children,
	keybind,
	variant,
	className,
	selected,
	to,
	...props
}: DropdownItemProps & RadixDM.MenuItemProps) => {
	const ref = useRef<HTMLDivElement>(null);

	return (
		<RadixDM.Item ref={ref} className={clsx(contextMenuItemClassNames, className)} {...props}>
			{to ? (
				<Link to={to} onClick={() => ref.current?.click()}>
					<ContextMenuDivItem
						className={clsx(selected && 'bg-accent text-white')}
						{...{ icon, iconProps, label, rightArrow, keybind, variant, children }}
					/>
				</Link>
			) : (
				<ContextMenuDivItem
					className={clsx(selected && 'bg-accent text-white')}
					{...{ icon, iconProps, label, rightArrow, keybind, variant, children }}
				/>
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
