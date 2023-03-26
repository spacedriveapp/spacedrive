import * as Radix from '@radix-ui/react-popover';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';

interface Props extends Radix.PopoverContentProps {
	trigger: React.ReactNode;
	disabled?: boolean;
}

export const Popover = ({
	trigger,
	children,
	disabled,
	className,
	...props
}: PropsWithChildren<Props>) => {
	return (
		<Radix.Root>
			<Radix.Trigger disabled={disabled} asChild>
				{trigger}
			</Radix.Trigger>

			<Radix.Portal>
				<Radix.Content
					className={clsx(
						'flex flex-col',
						'z-50 m-2 min-w-[11rem]',
						'cursor-default select-none rounded-lg',
						'text-ink text-left text-sm',
						'bg-app-overlay ',
						'border-app-line border',
						'shadow-2xl',
						'animate-in fade-in',
						className
					)}
					{...props}
				>
					{children}
				</Radix.Content>
			</Radix.Portal>
		</Radix.Root>
	);
};

export { Close as PopoverClose } from '@radix-ui/react-popover';
