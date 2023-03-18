import * as RadixCM from '@radix-ui/react-context-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { CaretRight, Icon, IconProps } from 'phosphor-react';
import { PropsWithChildren, Suspense } from 'react';

interface ContextMenuProps extends RadixCM.MenuContentProps {
	trigger: React.ReactNode;
}

export const contextMenuClasses = clsx(
	'z-50 flex flex-col',
	'my-2 min-w-[8rem] px-1 py-0.5',
	'text-menu-ink text-left text-sm',
	'bg-menu cool-shadow',
	'border-menu-line border',
	'cursor-default select-none rounded-md'
);

const Root = ({ trigger, children, className, ...props }: PropsWithChildren<ContextMenuProps>) => {
	return (
		<RadixCM.Root>
			<RadixCM.Trigger asChild>{trigger}</RadixCM.Trigger>
			<RadixCM.Portal>
				<RadixCM.Content {...props} className={clsx(contextMenuClasses, className)}>
					{children}
				</RadixCM.Content>
			</RadixCM.Portal>
		</RadixCM.Root>
	);
};

export const contextMenuSeparatorClassNames =
	'border-b-menu-line pointer-events-none mx-2 my-1 border-0 border-b';

const Separator = (props: { className?: string }) => (
	<RadixCM.Separator className={clsx(contextMenuSeparatorClassNames, props.className)} />
);

export const contextSubMenuTriggerClassNames =
	"[&[data-state='open']_div]:bg-accent text-menu-ink py-[3px]  focus:outline-none [&[data-state='open']_div]:text-white";

const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixCM.MenuSubContentProps & ContextMenuItemProps) => {
	return (
		<RadixCM.Sub>
			<RadixCM.SubTrigger className={contextSubMenuTriggerClassNames}>
				<DivItem rightArrow {...{ label, icon }} />
			</RadixCM.SubTrigger>
			<RadixCM.Portal>
				<Suspense fallback={null}>
					<RadixCM.SubContent {...props} className={clsx(contextMenuClasses, '-mt-2', className)} />
				</Suspense>
			</RadixCM.Portal>
		</RadixCM.Sub>
	);
};

export const contextMenuItemStyles = cva(
	[
		'flex flex-1 flex-row items-center justify-start overflow-hidden',
		'space-x-2 px-2 py-[3px]',
		'cursor-default rounded',
		'focus:outline-none'
	],
	{
		variants: {
			variant: {
				default: 'hover:bg-accent focus:bg-accent hover:text-white',
				danger: [
					'text-red-600 dark:text-red-400',
					'hover:text-white focus:text-white',
					'hover:bg-red-500 focus:bg-red-500'
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

const Item = ({
	icon,
	label,
	rightArrow,
	children,
	keybind,
	variant,
	...props
}: ContextMenuItemProps & RadixCM.MenuItemProps) => {
	return (
		<RadixCM.Item {...props} className="">
			<div className={contextMenuItemStyles({ variant })}>
				{children ? children : <ItemInternals {...{ icon, label, rightArrow, keybind }} />}
			</div>
		</RadixCM.Item>
	);
};

const DivItem = ({ variant, ...props }: ContextMenuItemProps) => (
	<div className={contextMenuItemStyles({ variant })}>
		<ItemInternals {...props} />
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
			{label && <p>{label}</p>}

			{keybind && (
				<span className="flex-end text-menu-faint absolute right-3 text-xs font-medium group-hover:text-white">
					{keybind}
				</span>
			)}
			{rightArrow && (
				<>
					<div className="flex-1" />
					<CaretRight weight="fill" size={12} alt="" className="text-menu-faint" />
				</>
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
