import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import { PropsWithChildren, useState } from 'react';
import { animated, config, useTransition } from 'react-spring';

interface Props extends DropdownMenu.MenuContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
}

const MENU_CLASSES = `
  flex flex-col
  min-w-[11rem] z-50 m-2 space-y-1
  text-left text-sm dark:text-gray-100 text-gray-800
  bg-gray-50 border-gray-200 dark:bg-gray-600
  border border-gray-300 dark:border-gray-500
  shadow-2xl shadow-gray-300 dark:shadow-gray-950 
  select-none cursor-default rounded-lg 
	!bg-opacity-80 backdrop-blur
`;

export const OverlayPanel = ({
	trigger,
	children,
	disabled,
	transformOrigin,
	className,
	...props
}: PropsWithChildren<Props>) => {
	const [open, setOpen] = useState(false);

	const transitions = useTransition(open, {
		from: {
			opacity: 0,
			transform: `scale(${0.9})`,
			transformOrigin: transformOrigin || 'top'
		},
		enter: { opacity: 1, transform: 'scale(1)' },
		leave: { opacity: -0.5, transform: 'scale(0.95)' },
		config: { mass: 0.4, tension: 200, friction: 10 }
	});

	return (
		<DropdownMenu.Root open={open} onOpenChange={setOpen}>
			<DropdownMenu.Trigger disabled={disabled} asChild>
				{trigger}
			</DropdownMenu.Trigger>
			{transitions(
				(styles, show) =>
					show && (
						<DropdownMenu.Portal forceMount>
							<DropdownMenu.Content forceMount asChild>
								<animated.div className={clsx(MENU_CLASSES, className)} style={styles}>
									{children}
								</animated.div>
							</DropdownMenu.Content>
						</DropdownMenu.Portal>
					)
			)}
		</DropdownMenu.Root>
	);
};
