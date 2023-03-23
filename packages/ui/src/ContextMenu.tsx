import * as RadixCM from '@radix-ui/react-context-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { CaretRight, Icon, IconProps } from 'phosphor-react';
import { PropsWithChildren, Suspense } from 'react';

interface ContextMenuProps extends RadixCM.MenuContentProps {
	trigger: React.ReactNode;
}

export const contextMenuClassNames = clsx(
	'z-50 max-h-[calc(100vh-20px)] overflow-y-auto',
	'my-2 min-w-[12rem] max-w-[16rem] px-1 py-0.5',
	'bg-menu cool-shadow',
	'border-menu-line border',
	'cursor-default select-none rounded-md'
);

const Root = ({ trigger, children, className, ...props }: ContextMenuProps) => {
	return (
		<RadixCM.Root>
			<RadixCM.Trigger asChild>{trigger}</RadixCM.Trigger>
			<RadixCM.Portal>
				<RadixCM.Content className={clsx(contextMenuClassNames, className)} {...props}>
					{children}
				</RadixCM.Content>
			</RadixCM.Portal>
		</RadixCM.Root>
	);
};

export const contextMenuSeparatorClassNames = 'border-b-menu-line my-0.5 border-b';

const Separator = (props: { className?: string }) => (
	<RadixCM.Separator className={clsx(contextMenuSeparatorClassNames, props.className)} />
);

const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixCM.MenuSubContentProps & ContextMenuItemProps) => {
	return (
		<RadixCM.Sub>
			<RadixCM.SubTrigger className={contextMenuItemClassNames}>
				<ContextMenuDivItem rightArrow {...{ label, icon }} />
			</RadixCM.SubTrigger>
			<RadixCM.Portal>
				<Suspense fallback={null}>
					<RadixCM.SubContent
						className={clsx(contextMenuClassNames, '-mt-2', className)}
						{...props}
					/>
				</Suspense>
			</RadixCM.Portal>
		</RadixCM.Sub>
	);
};

export const contextMenuItemStyles = cva(
	[
		'flex h-[26px] items-center space-x-2 overflow-hidden rounded px-2',
		'text-ink text-sm',
		'group-radix-highlighted:text-white dark:group-radix-highlighted:text-ink',
		'group-radix-highlighted:text-white dark:group-radix-highlighted:text-ink',
		'group-radix-disabled:text-ink/50 group-radix-disabled:pointer-events-none',
		'group-radix-state-open:bg-accent group-radix-state-open:text-white dark:group-radix-state-open:text-ink'
	],
	{
		variants: {
			variant: {
				default: 'group-radix-highlighted:bg-accent',
				danger: [
					'text-red-600 dark:text-red-400',
					'group-radix-highlighted:text-white',
					'group-radix-highlighted:bg-red-500'
				]
			}
		},
		defaultVariants: {
			variant: 'default'
		}
	}
);

export interface ContextMenuItemProps extends VariantProps<typeof contextMenuItemStyles> {
	icon?: Icon;
	iconProps?: IconProps;
	rightArrow?: boolean;
	label?: string;
	keybind?: string;
}

export const contextMenuItemClassNames = 'group py-0.5 outline-none';

const Item = ({
	icon,
	label,
	rightArrow,
	children,
	keybind,
	variant,
	iconProps,
	...props
}: ContextMenuItemProps & RadixCM.MenuItemProps) => {
	return (
		<RadixCM.Item className={contextMenuItemClassNames} {...props}>
			<ContextMenuDivItem {...{ icon, iconProps, label, rightArrow, keybind, variant, children }} />
		</RadixCM.Item>
	);
};

export const ContextMenuDivItem = ({
	variant,
	children,
	className,
	...props
}: PropsWithChildren<ContextMenuItemProps & { className?: string }>) => (
	<div className={contextMenuItemStyles({ variant, className })}>
		{children || <ItemInternals {...props} />}
	</div>
);

export const ItemInternals = ({
	icon,
	label,
	rightArrow,
	keybind,
	iconProps
}: ContextMenuItemProps) => {
	const ItemIcon = icon;

	return (
		<>
			{ItemIcon && <ItemIcon size={18} {...iconProps} />}
			{label && <span className="flex-1 truncate">{label}</span>}

			{keybind && (
				<span className="text-menu-faint group-radix-highlighted:text-white text-xs font-medium">
					{keybind}
				</span>
			)}
			{rightArrow && (
				<CaretRight
					weight="fill"
					size={12}
					className="text-menu-faint group-radix-highlighted:text-white group-radix-state-open:text-white"
				/>
			)}
		</>
	);
};

export const ContextMenu = {
	Root,
	Item,
	Separator,
	SubMenu
};
