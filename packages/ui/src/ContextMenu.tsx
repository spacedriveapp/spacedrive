import * as RadixCM from '@radix-ui/react-context-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { CaretRight, Icon } from 'phosphor-react';
import { PropsWithChildren, Suspense } from 'react';

interface Props extends RadixCM.MenuContentProps {
	trigger: React.ReactNode;
}

const MENU_CLASSES = `
  flex flex-col z-50
  min-w-[8rem] px-1 py-0.5
  text-left text-sm text-menu-ink
  bg-menu cool-shadow
	border border-menu-line
  select-none cursor-default rounded-md
`;

export const ContextMenu = ({
	trigger,
	children,
	className,
	...props
}: PropsWithChildren<Props>) => {
	return (
		<RadixCM.Root>
			<RadixCM.Trigger asChild>{trigger}</RadixCM.Trigger>
			<RadixCM.Portal>
				<RadixCM.Content {...props} className={clsx(MENU_CLASSES, className)}>
					{children}
				</RadixCM.Content>
			</RadixCM.Portal>
		</RadixCM.Root>
	);
};

export const Separator = () => (
	<RadixCM.Separator className="border-b-menu-line pointer-events-none mx-2 border-0 border-b" />
);

export const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixCM.MenuSubContentProps & ItemProps) => {
	return (
		<RadixCM.Sub>
			<RadixCM.SubTrigger className="[&[data-state='open']_div]:bg-accent text-menu-ink py-[3px]  focus:outline-none [&[data-state='open']_div]:text-white">
				<DivItem rightArrow {...{ label, icon }} />
			</RadixCM.SubTrigger>
			<RadixCM.Portal>
				<Suspense fallback={null}>
					<RadixCM.SubContent {...props} className={clsx(MENU_CLASSES, '-mt-2', className)} />
				</Suspense>
			</RadixCM.Portal>
		</RadixCM.Sub>
	);
};

const itemStyles = cva(
	[
		'flex flex-row items-center justify-start flex-1',
		'px-2 py-[3px] space-x-2',
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

interface ItemProps extends VariantProps<typeof itemStyles> {
	icon?: Icon;
	rightArrow?: boolean;
	label?: string;
	keybind?: string;
}

export const Item = ({
	icon,
	label,
	rightArrow,
	children,
	keybind,
	variant,

	...props
}: ItemProps & RadixCM.MenuItemProps) => {
	return (
		<RadixCM.Item
			{...props}
			className="text-menu-ink group !cursor-default  select-none py-0.5 focus:outline-none active:opacity-80"
		>
			<div className={itemStyles({ variant })}>
				{children ? children : <ItemInternals {...{ icon, label, rightArrow, keybind }} />}
			</div>
		</RadixCM.Item>
	);
};

const DivItem = ({ variant, ...props }: ItemProps) => (
	<div className={itemStyles({ variant })}>
		<ItemInternals {...props} />
	</div>
);

const ItemInternals = ({ icon, label, rightArrow, keybind }: ItemProps) => {
	const ItemIcon = icon;
	return (
		<>
			{ItemIcon && <ItemIcon size={18} />}
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
