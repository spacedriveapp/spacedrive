import * as RadixCM from '@radix-ui/react-context-menu';
import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { CaretRight, Icon } from 'phosphor-react';
import { PropsWithChildren, Suspense } from 'react';

interface Props extends RadixCM.MenuContentProps {
	trigger: React.ReactNode;
}

const MENU_CLASSES = `
  flex flex-col
  min-w-[11rem] p-2 space-y-1
  text-left text-sm dark:text-gray-100 text-gray-800
  bg-gray-50 border-gray-200 dark:bg-gray-750 dark:bg-opacity-70 backdrop-blur
	border border-transparent dark:border-gray-550
  shadow-md shadow-gray-300 dark:shadow-gray-750 
  select-none cursor-default rounded-lg 
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
	<RadixCM.Separator className="mx-2 border-0 border-b pointer-events-none border-b-gray-300 dark:border-b-gray-600" />
);

export const SubMenu = ({
	label,
	icon,
	className,
	...props
}: RadixCM.MenuSubContentProps & ItemProps) => {
	return (
		<RadixCM.Sub>
			<RadixCM.SubTrigger className="[&[data-state='open']_div]:bg-primary focus:outline-none">
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

const ITEM_CLASSES = `
  flex flex-row items-center justify-start flex-1 
  px-2 py-1 space-x-2
  cursor-default rounded
  focus:outline-none
`;

const itemStyles = cva([ITEM_CLASSES], {
	variants: {
		variant: {
			default: 'hover:bg-primary focus:bg-primary',
			danger: `
        text-red-600 dark:text-red-400
        hover:text-white focus:text-white
        hover:bg-red-500 focus:bg-red-500
      `
		}
	},
	defaultVariants: {
		variant: 'default'
	}
});

interface ItemProps extends VariantProps<typeof itemStyles> {
	icon?: Icon;
	rightArrow?: boolean;
	label?: string;
}

export const Item = ({
	icon,
	label,
	rightArrow,
	children,
	variant,
	...props
}: ItemProps & RadixCM.MenuItemProps) => (
	<RadixCM.Item {...props} className={itemStyles({ variant })}>
		{children ? children : <ItemInternals {...{ icon, label, rightArrow }} />}
	</RadixCM.Item>
);

const DivItem = ({ variant, ...props }: ItemProps) => (
	<div className={itemStyles({ variant })}>
		<ItemInternals {...props} />
	</div>
);

const ItemInternals = ({ icon, label, rightArrow }: ItemProps) => {
	const ItemIcon = icon;
	return (
		<>
			{ItemIcon && <ItemIcon size={18} />}
			{label && <p>{label}</p>}

			{rightArrow && (
				<>
					<div className="flex-1" />
					<CaretRight weight="fill" size={12} alt="" />
				</>
			)}
		</>
	);
};
